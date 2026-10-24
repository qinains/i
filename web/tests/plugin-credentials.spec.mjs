import {test,expect} from './fixtures.mjs';
import {readFile} from 'node:fs/promises';

test('saved credentials are not included in guest frames or exposed by host operations',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();
    const bytes=[...await readFile(new URL('../../target/browser-plugin-fixtures/example.word-count-1.0.0.hplugin.zip',import.meta.url))];
    const result=await page.evaluate(async bytes=>{
        const secret='PLUGIN_ISOLATION_FAKE_KEY',relay='PLUGIN_ISOLATION_FAKE_RELAY';
        const {saveConnection}=await import('/connections.js'),{DEFAULT_CONFIGURATION}=await import('/config.js');
        saveConnection(localStorage,'default',{...DEFAULT_CONFIGURATION,base_url:'https://model.invalid',model:'fixture',api_key:secret,relay_token:relay});
        const {inspectPluginPackage}=await import('/plugins/runner.js'),{runPluginTask}=await import('/plugins/agent.js');
        const {createPluginBroker}=await import('/plugins/broker.js'),{openPluginStore}=await import('/plugins/store.js');
        const {opfsOpenWorkspace,opfsWrite}=await import('/platform.js');
        const verified=await inspectPluginPackage(new Uint8Array(bytes)),descriptor=verified.descriptors[0],snapshot={workspaceId:'default',tools:[descriptor]};
        const root=await opfsOpenWorkspace('default');await opfsWrite(root,'notes.txt',new TextEncoder().encode('one two'));
        const store=await openPluginStore(),frames=[],original=Worker.prototype.postMessage;
        Worker.prototype.postMessage=function(message,...rest){frames.push({type:message.type,request:message.request,descriptor:message.descriptor,response:message.response});return original.call(this,message,...rest);};
        try{
            let callback;const agent={api_key:secret,relay_token:relay,set_plugin_tools:(_,fn)=>{callback=fn;},run:()=>callback(descriptor.publicName,JSON.stringify({path:'notes.txt'}))};
            const output=await runPluginTask({agent,manager:{withSnapshot:(_,run)=>run(snapshot),loadModule:async()=>verified.wasm},store,record:{workspaceId:'default',id:'session',mode:'auto'},text:'count',signal:new AbortController().signal});
            const privileged={...descriptor,permissions:{workspace:'read-write',storage:true}},broker=createPluginBroker({snapshot:{workspaceId:'default',tools:[privileged]},descriptor:privileged,store,context:{workspaceId:'default',mode:'auto'}});
            const denied=[];let id=0;
            for(const op of ['config.get','credentials.get','localStorage.getItem','indexedDB.get','fetch'])denied.push(await broker({id:++id,op,args:{key:'i_harness_profiles'}}));
            denied.push(await broker({id:++id,op:'fs.read',args:{path:'../i_harness_profiles'}}));
            const privateValue=await broker({id:++id,op:'kv.get',args:{key:'i_harness_profiles'}});
            return {output,frames,denied,privateValue,saved:localStorage.getItem('i_harness_profiles').includes(secret)};
        }finally{Worker.prototype.postMessage=original;store.close();}
    },bytes);
    expect(result.saved).toBe(true);expect(JSON.stringify(result.frames)).not.toMatch(/PLUGIN_ISOLATION_FAKE/);
    expect(result.frames.find(frame=>frame.type==='run').request).toMatchObject({event:'tool',data:{arguments:{path:'notes.txt'}}});
    expect(JSON.parse(result.output.output).words).toBe(2);expect(result.denied.every(reply=>reply.ok===false)).toBe(true);
    expect(result.privateValue).toMatchObject({ok:true,result:{value:null}});
});

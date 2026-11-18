import {test,expect} from './fixtures.mjs';
test('plugin broker enforces workspace capabilities and commits real OPFS and scoped KV',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const result=await page.evaluate(async()=>{
        const {createPluginBroker}=await import('/plugins/broker.js'),{openPluginStore}=await import('/plugins/store.js');const store=await openPluginStore();
        const descriptor={pluginId:'example.test',version:'1.0.0',digest:'a'.repeat(64),publicName:'p_test_0000000000000000',permissions:{workspace:'read-write',storage:true}};
        const snapshot={workspaceId:'default',tools:[descriptor]},context={workspaceId:'default',sessionId:'test',mode:'auto',signal:new AbortController().signal,approve:async()=>true,pauseBudget:()=>{}};
        const broker=createPluginBroker({snapshot,descriptor,context,store});
        const write=await broker({id:1,op:'fs.write',args:{path:'/workspace/plugin/result.txt',text:'persistent plugin'}});
        const read=await broker({id:2,op:'fs.read',args:{path:'plugin/result.txt'}});
        const traversal=await broker({id:3,op:'fs.read',args:{path:'../outside'}});
        const network=await broker({id:4,op:'fetch',args:{url:'https://outside.invalid'}});
        await broker({id:5,op:'kv.put',args:{key:'x',value:{answer:42}}});const kv=await broker({id:6,op:'kv.get',args:{key:'x'}});
        const readonly={...descriptor,permissions:{workspace:'read',storage:false}};
        const denied=await createPluginBroker({snapshot:{workspaceId:'default',tools:[readonly]},descriptor,context,store})({id:1,op:'fs.write',args:{path:'no.txt',text:'no'}});
        store.close();return {write:write.ok,text:read.result.text,traversal:traversal.ok,network:network.ok,kv:kv.result.value,denied:denied.ok};
    });
    expect(result).toEqual({write:true,text:'persistent plugin',traversal:false,network:false,kv:{answer:42},denied:false});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    expect(await page.evaluate(async()=>{const r=await(await navigator.storage.getDirectory()).getDirectoryHandle('workspace'),d=await r.getDirectoryHandle('plugin');return(await(await d.getFileHandle('result.txt')).getFile()).text();})).toBe('persistent plugin');
});

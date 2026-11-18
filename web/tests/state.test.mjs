import test from 'node:test';
import assert from 'node:assert/strict';
import {recoverHistory} from '../state/recovery.js';
import {legacyWorkspace} from '../state/migration.js';
import {validateSession} from '../state/records.js';
import {SessionController} from '../sessions.js';
import {Context} from '../plugins/vendor/cordis.js';
test('overlapping directory refreshes reuse a single agent initialization',async()=>{
    let release,created=0;const gate=new Promise(resolve=>release=resolve),instances=[];
    const Agent={create_workspace:async()=>{created++;await gate;const agent={import_history(){},cancel(){},async close(){},free(){}};instances.push(agent);return agent;}};
    const record={id:'init',workspaceId:'default',history:[{role:'system',content:'system'}]};
    const context=new Context(),controller=new SessionController(Agent,{},{},context),first=controller.agent(record),second=controller.agent(record);
    release();const [a,b]=await Promise.all([first,second]);assert.equal(created,1);assert.equal(a,b);assert.equal(controller.agents.get(record.id),a);
    await controller.shutdown();await context.fiber.dispose();
});
test('a saved new conversation restores before its lazy model history is initialized',async()=>{
    const errors=[],record={id:'a',workspaceId:'default',name:'Draft',titleSource:'manual',connectionId:'',connectionIdentity:'',draft:'keep draft',revision:1,history:[],transcript:[{type:'system',text:'keep display'}],status:'idle'};
    const controller=new SessionController({validate_history:json=>{assert.notEqual(json,'[]','WASM requires a system message, unlike an uninitialized conversation');}}, {}, {error:error=>errors.push(error)},new Context());
    await controller.restore([record]);assert.equal(controller.records.get('a'),record);assert.deepEqual(errors,[]);
});
test('invalid nonempty model history is retained but not opened and reports only its failure stage',async()=>{
    const errors=[],record={id:'a',workspaceId:'default',name:'Draft',titleSource:'manual',connectionId:'',connectionIdentity:'',draft:'',revision:1,history:[{role:'user',content:'PRIVATE'}],transcript:[],status:'idle'};
    const original=structuredClone(record);
    const controller=new SessionController({validate_history:()=>{throw new Error('PRIVATE');}}, {}, {error:error=>errors.push(error)},new Context());
    await controller.restore([record]);assert.equal(controller.records.size,0);assert.deepEqual(record,original);
    assert.match(errors[0].message,/stage: history/);assert.ok(!errors[0].message.includes('PRIVATE'));
});
test('legacy labels map to one default directory without pretending files are isolated',()=>{
    const values={i_workspaces:JSON.stringify({A:{},B:{}}),i_current_workspace:'B'};
    assert.deepEqual(legacyWorkspace({getItem:key=>values[key]}),{source:'i_workspaces',version:1,name:'B',labels:['A','B']});
});
test('corrupt transcript data is rejected before rendering',()=>{
    assert.throws(()=>validateSession({id:'a',workspaceId:'default',name:'a',connectionId:'',connectionIdentity:'',draft:'',revision:1,history:[],transcript:[null],status:'idle'}),/Invalid/);
});
test('old sessions default to ask while unknown persisted execution modes are rejected',()=>{
    const record={id:'a',workspaceId:'default',name:'a',connectionId:'',connectionIdentity:'',draft:'',revision:1,history:[],transcript:[],status:'idle'};
    assert.equal(validateSession(record).mode??'ask','ask');assert.equal(validateSession({...record,mode:'auto'}).mode,'auto');assert.throws(()=>validateSession({...record,mode:'unsafe'}),/Invalid/);
});
test('interrupted batch restores real results and explicit unknown/not-executed outcomes',()=>{
    const call=id=>({id,type:'function',function:{name:'write',arguments:'{}'}});
    const assistant={role:'assistant',content:null,tool_calls:[call('a'),call('b'),call('c')]};
    const history=recoverHistory([{role:'system',content:'test'}],{assistant,calls:[{id:'a',status:'finished',result:{output:'written'}},{id:'b',status:'started'},{id:'c',status:'pending'}]});
    assert.equal(history[2].content,'written');assert.match(history[3].content,/unknown/i);assert.match(history[4].content,/not executed/i);
});
const png='data:image/png;base64,iVBORw0KGgo=';
const recoveryCall=id=>({id,type:'function',function:{name:'view_image',arguments:'{}'}});
test('recovery restores finished image results after every tool result in assistant order',()=>{
    const base=[{role:'system',content:'test'}],assistant={role:'assistant',content:null,tool_calls:['a','b','c'].map(recoveryCall)};
    const image={type:'image_url',image_url:{url:png}},journal={assistant,calls:[{id:'b',status:'pending',result:{images:[image]}},{id:'a',status:'finished',result:{output:'Image attached.',images:[image]}}]};
    const before=structuredClone({base,journal});
    const restored=recoverHistory(base,journal);
    assert.deepEqual(restored.map(m=>m.role),['system','assistant','tool','tool','tool','user']);
    assert.deepEqual(restored.at(-1).content,[{type:'text',text:'Image from tool "a"'},image]);
    assert.match(restored[4].content,/not executed/i);
    assert.deepEqual({base,journal},before);
    restored.at(-1).content[1].image_url.url='changed';assert.equal(image.image_url.url,png);
});
test('recovery rejects corrupt journals and image data instead of replaying tools',()=>{
    const base=[{role:'system',content:'test'}],assistant={role:'assistant',content:null,tool_calls:[recoveryCall('a')]};
    const journal=result=>({assistant,calls:[{id:'a',status:'finished',result}]});
    for(const result of [{},{output:1},{output:'x',images:{}},{output:'x',images:[{type:'image_url',image_url:{url:'https://example.invalid/image.png'}}]},{output:'x',images:[{type:'image_url',image_url:{url:png.replace('image/png','image/jpeg')}}]},{output:'x',images:[{type:'image_url',image_url:{url:png+'!'}}]},{output:'x',images:Array(9).fill({type:'image_url',image_url:{url:png}})}])assert.throws(()=>recoverHistory(base,journal(result)));
    assert.throws(()=>recoverHistory(base,{assistant,calls:[{id:'other',status:'pending'}]}));
    assert.throws(()=>recoverHistory(base,{assistant,calls:[{id:'a',status:'pending'},{id:'a',status:'started'}]}));
    assert.throws(()=>recoverHistory(base,{assistant,calls:[{id:'a',status:'invalid'}]}));
});
test('recovered image limits apply across the whole batch, with strict canonical base64',()=>{
    const base=[{role:'system',content:'test'}],assistant={role:'assistant',tool_calls:['a','b'].map(recoveryCall)},image={type:'image_url',image_url:{url:png}};
    const journal={assistant,calls:[{id:'b',status:'finished',result:{output:'b',images:[image]}},{id:'a',status:'finished',result:{output:'a',images:[image]}}]};
    const images=recoverHistory(base,journal).at(-1).content;
    assert.equal(images[0].text,'Image from tool "a"');assert.equal(images[2].text,'Image from tool "b"');
    journal.calls[0].result.images=Array(8).fill(image);assert.throws(()=>recoverHistory(base,journal));
    journal.calls[0].result.images=[{type:'image_url',image_url:{url:png.slice(0,-1)}}];assert.throws(()=>recoverHistory(base,journal));
    const bytes=new Uint8Array(8*1024*1024+1);bytes.set([137,80,78,71,13,10,26,10]);
    journal.calls[0].result.images=[{type:'image_url',image_url:{url:'data:image/png;base64,'+Buffer.from(bytes).toString('base64')}}];
    assert.throws(()=>recoverHistory(base,journal));
});

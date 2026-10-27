import test from 'node:test';
import assert from 'node:assert/strict';
import {recoverHistory} from '../state/recovery.js';
import {legacyWorkspace} from '../state/migration.js';
import {validateSession} from '../state/records.js';
import {SessionController} from '../sessions.js';
test('overlapping directory refreshes reuse a single agent initialization',async()=>{
    let release,created=0;const gate=new Promise(resolve=>release=resolve),instances=[];
    const Agent={create_workspace:async()=>{created++;await gate;const agent={import_history:()=>{}};instances.push(agent);return agent;}};
    const record={id:'init',workspaceId:'default',history:[{role:'system',content:'system'}]};
    const controller=new SessionController(Agent,{},{}),first=controller.agent(record),second=controller.agent(record);
    release();const [a,b]=await Promise.all([first,second]);assert.equal(created,1);assert.equal(a,b);assert.equal(controller.agents.get(record.id),a);
});
test('a saved new conversation restores before its lazy model history is initialized',async()=>{
    const errors=[],record={id:'a',workspaceId:'default',name:'Draft',titleSource:'manual',connectionId:'',connectionIdentity:'',draft:'keep draft',revision:1,history:[],transcript:[{type:'system',text:'keep display'}],status:'idle'};
    const controller=new SessionController({validate_history:json=>{assert.notEqual(json,'[]','WASM requires a system message, unlike an uninitialized conversation');}}, {}, {error:error=>errors.push(error)});
    await controller.restore([record]);assert.equal(controller.records.get('a'),record);assert.deepEqual(errors,[]);
});
test('invalid nonempty model history is retained but not opened and reports only its failure stage',async()=>{
    const errors=[],record={id:'a',workspaceId:'default',name:'Draft',titleSource:'manual',connectionId:'',connectionIdentity:'',draft:'',revision:1,history:[{role:'user',content:'PRIVATE'}],transcript:[],status:'idle'};
    const original=structuredClone(record);
    const controller=new SessionController({validate_history:()=>{throw new Error('PRIVATE');}}, {}, {error:error=>errors.push(error)});
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
test('analysis reports persist as independent transcript entries, never model history',()=>{
    const record={id:'report',workspaceId:'default',name:'Original title',connectionId:'',connectionIdentity:'',draft:'',revision:1,history:[],transcript:[{type:'analysis',text:'Independent reflection'}],status:'idle'};
    assert.equal(validateSession(record),record);assert.deepEqual(record.history,[]);assert.equal(record.name,'Original title');
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

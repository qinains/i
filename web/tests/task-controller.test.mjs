import {test} from 'node:test';
import assert from 'node:assert/strict';
import {TaskController} from '../task-controller.js';
import {connectionIdentity} from '../sessions.js';

function fixture(save=async()=>{}){
    const record={id:'one',workspaceId:'default',mode:'ask',status:'idle',draft:'keep draft',composerBlocks:[],transcript:[]};
    const agent={cancel(){}};
    const config={base_url:'http://127.0.0.1:4173/mock/v1',protocol:'openai',model:'fixture',transport:'direct'};
    const sessions={conflicts:new Map(),save,flush:async()=>{},later(){},agent:async()=>agent,configure:async()=>agent};
    const tasks=new TaskController({sessions:()=>sessions,blocked:()=>false,changed(){},focus(){},append(){},report(){},refresh:async()=>{},
        online:()=>true,redact:text=>text,connection:()=>({id:'fixture',config}),plugins:async()=>({}),draftCleared(){assert.fail('no durable message was admitted');}},async(_id,fn)=>fn());
    return {tasks,record,sessions,config};
}

test('approval belongs to the admitted task and only its session can accept it',async()=>{
    const {tasks,record}=fixture(),waiting=Promise.withResolvers();
    const task=tasks.run(record,async()=>{
        const result=tasks.approve(record.id,'Write file');waiting.resolve();assert.equal(await result,true);
    });
    await waiting.promise;
    assert.equal(record.status,'approval');
    assert.deepEqual(tasks.pendingApproval,{id:'one',description:'Write file'});
    tasks.settleApproval(true,'other');assert.equal(record.status,'approval');
    await assert.rejects(tasks.run({...record,id:'other'},async()=>{}),/current task/);
    tasks.settleApproval(true,'one');assert.equal(await task,true);assert.equal(tasks.runningId,undefined);assert.equal(record.status,'idle');
});

test('cancelling during the approval checkpoint cannot show a late approval',async()=>{
    const checkpoint=Promise.withResolvers(),entered=Promise.withResolvers();
    const {record,tasks}=fixture(async r=>{if(r.status==='approval'){entered.resolve();await checkpoint.promise;}});
    const task=tasks.run(record,async()=>assert.equal(await tasks.approve(record.id,'Write file'),false));
    await entered.promise;tasks.cancel(record.id,{pause:true});checkpoint.resolve();
    assert.equal(await task,false);assert.equal(tasks.pendingApproval,undefined);assert.equal(record.queuePaused,true);
});

test('a failed admission save performs no task, even if the final save succeeds',async()=>{
    let writes=0,effects=0;const {tasks,record}=fixture(async()=>{if(++writes===1)throw new DOMException('write aborted','AbortError');});
    assert.equal(await tasks.run(record,async()=>{effects++;}),false);
    assert.equal(writes,2);assert.equal(effects,0);assert.equal(record.queuePaused,true);
});

test('a failed dispatch save retains the draft or queued item without starting a request',async()=>{
    for(const queued of [false,true]){
        let writes=0;
        const {tasks,record,sessions,config}=fixture(async()=>{if(++writes===2)throw new DOMException('write aborted','AbortError');});
        const entry={id:'queued',visible:'queued body',blocks:[],connectionId:'fixture',connectionIdentity:connectionIdentity(config)};
        if(queued)record.pendingMessages=[entry];
        let requests=0;
        sessions.configure=async()=>({set_plugin_tools(){},async run(){requests++;}});
        assert.equal(await tasks.send(record,queued?entry.visible:record.draft,[],queued?entry:null),false);
        assert.equal(record.draft,'keep draft');assert.deepEqual(record.transcript,[]);assert.equal(requests,0);
        if(queued)assert.deepEqual(record.pendingMessages,[entry]);
    }
});

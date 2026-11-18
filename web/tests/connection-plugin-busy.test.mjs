import test from 'node:test';
import assert from 'node:assert/strict';
import {TaskController} from '../task-controller.js';

test('queued messages cannot start inference during a provider account operation',async()=>{
    let sends=0,loginPending=true;
    const controller=new TaskController({blocked:()=>loginPending,sessions:()=>({}),changed(){}});
    const record={id:'test',queuePaused:false,pendingMessages:[{id:'one',visible:'private queued task'}]};
    controller.send=async()=>{sends++;record.pendingMessages=[];return true;};
    await controller.drain(record);assert.equal(sends,0);assert.equal(record.pendingMessages.length,1);
    loginPending=false;await controller.drain(record);assert.equal(sends,1);
});
test('workspace operations cannot bypass a pending provider login',async()=>{
    const controller=new TaskController({blocked:()=>true,sessions:()=>({})});
    await assert.rejects(controller.run({},async()=>assert.fail('blocked task ran')),/Finish or stop the current task first/u);
});

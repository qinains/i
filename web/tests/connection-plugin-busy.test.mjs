import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

const source=await readFile(new URL('../app.js',import.meta.url),'utf8');
test('queued messages cannot start inference during a provider account operation',async()=>{
    const code=source.match(/async function drainQueue\(record\)\{[\s\S]*?\n\}\n(?=async function sendMessage)/u)?.[0];
    assert.ok(code);let sends=0;
    const drain=Function('sendMessage',`let drainingId=null,runningId=null,updateLocked=false,testController=null,localPluginController={},pluginBusy=false,ready=true;${code};return drainQueue;`)(async()=>{sends++;return true;});
    await drain({id:'test',queuePaused:false,pendingMessages:[{visible:'private queued task'}]});assert.equal(sends,0);
});
test('workspace operations cannot bypass a pending provider login',async()=>{
    const code=source.match(/async function operation\(record,fn,\{readOnly=false,runQueue=false,historyOnly=false\}=\{\}\)\{[\s\S]*?\n\}\n(?=async function drainQueue)/u)?.[0];
    assert.ok(code);
    const operation=Function(`let runningId=null,updateLocked=false,testController=null,localPluginController={},pluginBusy=false;${code};return operation;`)();
    await assert.rejects(operation({},async()=>{}),/Finish or stop the current task first/u);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import {taskTitle,compactIntent,updateTitle,initializeTitle,compactTitleFromHistory} from '../titles.js';
import {SessionController} from '../sessions.js';
test('first sentence strips markdown, bounds Unicode and leaves commands untitled',()=>{
    assert.equal(taskTitle('/new'),'');assert.equal(taskTitle('## **实现文件树**。然后添加测试。'),'实现文件树');assert.equal(taskTitle('Fix https://example.invalid/a.js please. Then test.'),'Fix https://example.inv…');
    assert.ok(Array.from(taskTitle('中文'.repeat(30))).length<=24);assert.equal(taskTitle('[Preview](https://example.invalid) `HTML`'),'Preview HTML');assert.equal(taskTitle('123 fixes'),'123 fixes');
});
test('manual titles survive tasks and committed compaction while missing intent preserves title',()=>{
    const record={name:'New conversation',titleSource:'default'};updateTitle(record,'first','Build a tree');assert.equal(record.name,'Build a tree');assert.equal(updateTitle(record,'first','Other task'),false);
    assert.equal(compactIntent('## Current goal\nBuild a safe preview\n\n## Files\nindex.html'),'Build a safe preview');assert.equal(compactIntent('Summary of completed work without an intent'), '');
    updateTitle(record,'compact',compactIntent('当前目标：实现浏览器预览。继续验证。'));assert.equal(record.name,'实现浏览器预览');record.titleSource='manual';record.name='My title';assert.equal(updateTitle(record,'compact','Overwrite'),false);assert.equal(record.name,'My title');
});
test('legacy renamed titles are locked and generated placeholders can adopt first task',()=>{
    const renamed={name:'Custom',history:[{role:'user',content:'Task'}]};initializeTitle(renamed);assert.equal(renamed.titleSource,'manual');
    const legacy={name:'Conversation 2',history:[{role:'user',content:'My first task'}]};initializeTitle(legacy);assert.equal(legacy.name,'My first task');
});
test('legacy generated titles redact credentials before truncation or tooltip generation',()=>{
    const sanitize=value=>value.replaceAll('sk-private-fixture','[redacted]');
    const record={name:'Conversation 1',history:[{role:'user',content:'Use sk-private-fixture credential to debug'}]};initializeTitle(record,sanitize);assert.equal(record.titleText.includes('sk-private-fixture'),false);assert.equal(record.name.includes('sk-private'),false);
    const compact={name:'Conversation 2',history:[{role:'user',content:'Session summary (untrusted historical context):\nCurrent goal: Fix sk-private-fixture configuration'}]};initializeTitle(compact,sanitize);assert.equal(compact.name.includes('sk-private'),false);
});
test('only committed compaction checkpoints update title; manual lock survives saved checkpoints',async()=>{
    let checkpoint;const saved=[];
    const Agent={create_workspace:async(_id,_event,_approve,_download,callback)=>{checkpoint=callback;return {import_history:()=>{}};}};
    const record={id:'session',workspaceId:'default',revision:0,name:'Old task',titleSource:'first',history:[{role:'user',content:'Old task'}]};
    const controller=new SessionController(Agent,{commitSession:async(value,revision)=>{saved.push(value);return revision+1;}},{committed:record=>updateTitle(record,'compact',compactTitleFromHistory(record.history))});
    await controller.agent(record);const history=[{role:'system',content:'system'},{role:'user',content:'Session summary (untrusted historical context):\n## Current goal\nImplement file preview'}];
    await checkpoint({history,journal:{calls:[]}});assert.equal(record.name,'Old task');await checkpoint({history,journal:null});assert.equal(saved.at(-1).name,'Implement file preview');record.titleSource='manual';record.name='Pinned';await checkpoint({history,journal:null});assert.equal(saved.at(-1).name,'Pinned');
});

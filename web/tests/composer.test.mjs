import test from 'node:test';
import assert from 'node:assert/strict';
import {stageText,stageImage,composeDraft,removeBlock} from '../composer.js';
import {SessionController} from '../sessions.js';
const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=','base64');
test('long paste is compact but expands exactly once alongside a caption',()=>{
    const record={},text='中文\n'.repeat(500)+'[Image #1]';
    const token=stageText(record,text);assert.match(token,/Pasted Content.*chars/);
    assert.equal(composeDraft(record,`Review ${token}`).text,`Review ${text}`);
    assert.equal(stageText(record,'short'),'short');
});
test('folded text retains leading indentation and trailing newlines',()=>{
    const record={},text='  indented\n'.repeat(200);const token=stageText(record,text);
    assert.equal(composeDraft(record,token).text,text);
});
test('even a short multiline paste folds so a single-line input cannot discard newlines',()=>{
    const record={},token=stageText(record,'one\ntwo');assert.match(token,/Pasted Content/);
    assert.equal(composeDraft(record,token).text,'one\ntwo');
});
test('image-only drafts have actual blobs, are session scoped and can be removed',async()=>{
    const record={},other={};const token=await stageImage(record,new Blob([png],{type:'image/png'}));
    assert.equal(token,'[Image #1]');assert.equal(composeDraft(record,token).images.length,1);
    assert.equal(composeDraft(record,token).text,'');assert.equal(composeDraft(other,token).images.length,0);
    removeBlock(record,token);assert.equal(composeDraft(record,token).images.length,0);
});
test('fake image MIME and excessive images are rejected without changing drafts',async()=>{
    const record={};await assert.rejects(stageImage(record,new Blob(['fake'],{type:'image/png'})));
    for(let i=0;i<8;i++)await stageImage(record,new Blob([png],{type:'image/png'}));
    await assert.rejects(stageImage(record,new Blob([png],{type:'image/png'})));assert.equal(record.composerBlocks.length,8);
});
test('ordinary typed placeholders are never attachments and oversized expanded text is rejected',()=>{
    assert.deepEqual(composeDraft({},'[Image #1]'),{text:'[Image #1]',images:[]});
    assert.throws(()=>stageText({},'x'.repeat(65537)));
    const record={},token=stageText(record,'x'.repeat(64000));
    assert.throws(()=>composeDraft(record,`${token}${token}`));
});
test('saved indicator cannot overtake a newly staged attachment draft',async()=>{
    let release;const statuses=[];
    const controller=new SessionController(null,{commitSession:()=>new Promise(resolve=>{release=resolve;})},{saving:(_id,status)=>statuses.push(status)});
    const record={id:'draft',workspaceId:'default',revision:0};
    controller.records.set(record.id,record);
    const first=controller.save(record);await new Promise(setImmediate);
    controller.later(record);release(1);await first;
    assert.equal(statuses.at(-1),'Saving…');
    const flush=controller.flush(record.id);await new Promise(setImmediate);release(2);await flush;
    assert.equal(statuses.at(-1),'Saved');
});

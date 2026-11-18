import test from 'node:test';
import assert from 'node:assert/strict';
import {enqueueMessage,editMessage,removeMessage,validatePending} from '../pending.js';
import {composeDraft} from '../composer.js';
const connection={connectionId:'connection',connectionIdentity:'identity'};
test('pending messages own independent snapshots of pasted text and images',()=>{
    const record={composerBlocks:[{kind:'image',token:'[Image #1]',blob:new Blob(['image'],{type:'image/png'})},{kind:'text',token:'[Pasted Content #1]',text:'full pasted text'}]};
    const entry=enqueueMessage(record,'[Image #1] [Pasted Content #1] caption',connection);
    record.composerBlocks=[];
    assert.equal(composeDraft({composerBlocks:entry.blocks},entry.visible).text,' full pasted text caption');
    assert.equal(composeDraft({composerBlocks:entry.blocks},entry.visible).images.length,1);
    assert.ok(!JSON.stringify(entry).includes('api_key'));
    editMessage(record,entry.id,'[Image #1] edited');assert.equal(entry.visible,'[Image #1] edited');
    assert.equal(removeMessage(record,entry.id),true);assert.equal(record.pendingMessages.length,0);
});
test('invalid queue edits and commands do not destroy pending content',()=>{
    const record={};const entry=enqueueMessage(record,'original',connection);
    for(const text of ['', '/new', '!shell', 'x'.repeat(65537)])assert.throws(()=>editMessage(record,entry.id,text));
    assert.equal(entry.visible,'original');
    assert.throws(()=>enqueueMessage(record,'/new',connection));
    assert.equal(record.pendingMessages.length,1);
});
test('queues are bounded and restore validation rejects malformed records',()=>{
    const record={};for(let i=0;i<20;i++)enqueueMessage(record,`message ${i}`,connection);
    assert.throws(()=>enqueueMessage(record,'overflow',connection),/20/);
    validatePending(record);
    assert.throws(()=>validatePending({pendingMessages:[{visible:'invalid'}]}));
    assert.throws(()=>validatePending({pendingMessages:[record.pendingMessages[0],record.pendingMessages[0]]}));
});

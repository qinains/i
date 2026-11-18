import {test} from 'node:test';
import assert from 'node:assert/strict';
import {draftBackup} from '../state/draft-backup.js';

test('backup stores only unsent content and preserves attachments across base64 chunk boundaries',async()=>{
    const bytes=Buffer.alloc(48*1024+1,0x9f);bytes[0]=0x89;
    const image={kind:'image',token:'[Image #1]',blob:new Blob([bytes],{type:'image/png'})};
    const text={kind:'text',token:'[Pasted #1]',text:'text DUMMY_SECRET'};
    const record={draft:'draft DUMMY_SECRET',composerBlocks:[image,text],pendingMessages:[
        {visible:'next queued message',editDraft:'unsaved edit',blocks:[image],connectionId:'hidden connection id'},
    ],history:[{content:'hidden history'}],api_key:'hidden setting',journal:{status:'started'}};
    const blob=await draftBackup(record,value=>value.replaceAll('DUMMY_SECRET','[redacted]'));
    const raw=await blob.text(),backup=JSON.parse(raw);
    assert.deepEqual(Object.keys(backup),['format','version','draft','attachments','queue']);
    assert.equal(backup.version,1);assert.equal(backup.draft,'draft [redacted]');
    assert.deepEqual(backup.attachments[1],{...text,text:'text [redacted]'});
    assert.deepEqual(Buffer.from(backup.attachments[0].base64,'base64'),bytes);
    assert.deepEqual(backup.queue[0],{text:'next queued message',editDraft:'unsaved edit',attachments:[backup.attachments[0]]});
    assert.doesNotMatch(raw,/DUMMY_SECRET|hidden|started/);
    assert.equal(record.pendingMessages.length,1);
});

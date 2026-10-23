import {composeDraft} from './composer.js';
function validMessage(visible,blocks){
    if(typeof visible!=='string'||!visible.trim())throw new Error('Message is empty.');
    if(/^[\/!]/u.test(visible.trimStart()))throw new Error('Commands cannot be queued. Send a message instead.');
    return composeDraft({composerBlocks:blocks},visible);
}
export function validatePending(record){
    const entries=record.pendingMessages??[];
    if(!Array.isArray(entries)||entries.length>20||record.queuePaused!=null&&typeof record.queuePaused!=='boolean')throw new Error('Invalid pending messages.');
    const ids=new Set();let bytes=0;
    for(const entry of entries){
        if(!entry||typeof entry.id!=='string'||ids.has(entry.id)||typeof entry.connectionId!=='string'||typeof entry.connectionIdentity!=='string'||!Array.isArray(entry.blocks)||entry.blocks.length>64||entry.editDraft!=null&&typeof entry.editDraft!=='string')throw new Error('Invalid pending message.');
        ids.add(entry.id);
        for(const block of entry.blocks){
            if(!block||typeof block.token!=='string'||!block.token||!['image','text'].includes(block.kind)||block.kind==='text'&&typeof block.text!=='string'||block.kind==='image'&&!(block.blob instanceof Blob))throw new Error('Invalid queued attachment.');
            if(block.kind==='image')bytes+=block.blob.size;
        }
        validMessage(entry.visible,entry.blocks);
    }
    if(bytes>64*1024*1024)throw new Error('Queued images exceed 64 MiB.');
}
export function enqueueMessage(record,visible,{connectionId,connectionIdentity}){
    const blocks=structuredClone((record.composerBlocks??[]).filter(block=>visible.includes(block.token)));
    validMessage(visible,blocks);
    const entry={id:crypto.randomUUID(),visible,blocks,connectionId,connectionIdentity};
    const pendingMessages=[...(record.pendingMessages??[]),entry];
    if(pendingMessages.length>20)throw new Error('At most 20 queued messages.');
    validatePending({...record,pendingMessages});record.pendingMessages=pendingMessages;return entry;
}
export function editMessage(record,id,visible){
    const entry=record.pendingMessages?.find(item=>item.id===id);if(!entry)throw new Error('Queued message no longer exists.');
    validMessage(visible,entry.blocks);entry.visible=visible;delete entry.editDraft;
}
export function removeMessage(record,id){
    const entries=record.pendingMessages??[],next=entries.filter(entry=>entry.id!==id);
    record.pendingMessages=next;return entries.length!==next.length;
}

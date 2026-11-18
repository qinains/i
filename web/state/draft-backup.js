const CHUNK_BYTES=48*1024; // A multiple of three, so only the final base64 chunk has padding.
const MIME_TYPES=new Set(['image/png','image/jpeg','image/gif','image/webp']);

export async function draftBackup(record,redact){
    const snapshot=structuredClone({draft:record.draft,blocks:record.composerBlocks??[],queue:record.pendingMessages??[]});
    const parts=['{"format":"i-unsent-session","version":1,"draft":',JSON.stringify(redact(snapshot.draft??'')),',"attachments":'];
    let bytes=0;
    async function writeAttachments(blocks){
        parts.push('[');
        for(const [index,block] of blocks.entries()){
            if(index)parts.push(',');
            const {kind,token}=block;
            if(kind==='text'){parts.push(JSON.stringify({kind,token:redact(token),text:redact(block.text)}));continue;}
            const blob=block.blob;
            if(kind!=='image'||!(blob instanceof Blob)||!MIME_TYPES.has(blob.type)||blob.size>8*1024*1024||(bytes+=blob.size)>80*1024*1024)throw new Error('Invalid or oversized saved image.');
            parts.push('{"kind":"image","token":',JSON.stringify(redact(token)),',"mime":',JSON.stringify(blob.type),',"base64":"');
            for(let offset=0;offset<blob.size;offset+=CHUNK_BYTES){
                const chunk=new Uint8Array(await blob.slice(offset,offset+CHUNK_BYTES).arrayBuffer());
                parts.push(btoa(String.fromCharCode(...chunk)));
            }
            parts.push('"}');
        }
        parts.push(']');
    }
    await writeAttachments(snapshot.blocks);parts.push(',"queue":[');
    for(const [index,entry] of snapshot.queue.entries()){
        if(index)parts.push(',');
        parts.push('{"text":',JSON.stringify(redact(entry.visible)));
        if(entry.editDraft!=null)parts.push(',"editDraft":',JSON.stringify(redact(entry.editDraft)));
        parts.push(',"attachments":');await writeAttachments(entry.blocks);parts.push('}');
    }
    parts.push(']}');return new Blob(parts,{type:'application/json'});
}

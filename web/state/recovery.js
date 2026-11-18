function toolCalls(message){
    if(message.tool_calls!=null&&message.role!=='assistant')throw new Error('Invalid recovery tool role.');
    const calls=message.tool_calls??[];
    if(!Array.isArray(calls)||calls.length>128)throw new Error('Invalid recovery tool batch.');
    const ids=new Set();
    for(const call of calls){
        if(!call||typeof call.id!=='string'||!call.id||ids.has(call.id)||call.type!=='function'||typeof call.function?.name!=='string'||!call.function.name||typeof call.function.arguments!=='string')throw new Error('Invalid recovery tool call.');
        const args=JSON.parse(call.function.arguments);if(!args||typeof args!=='object'||Array.isArray(args))throw new Error('Invalid recovery tool arguments.');
        ids.add(call.id);
    }
    return calls;
}
function validateHistory(history){
    if(!Array.isArray(history)||history[0]?.role!=='system')throw new Error('Invalid recovery history.');
    const pending=new Set();
    for(const message of history){
        if(message?.role==='tool'){
            if(!pending.delete(message.tool_call_id))throw new Error('Invalid recovery tool result.');
        }else{
            if(pending.size||!['system','user','assistant'].includes(message?.role))throw new Error('Incomplete recovery tool batch.');
            const calls=toolCalls(message);if(calls.length&&message.role!=='assistant')throw new Error('Invalid recovery tool role.');
            for(const call of calls)pending.add(call.id);
        }
    }
    if(pending.size)throw new Error('Incomplete recovery tool batch.');
}
function savedImage(image){
    const url=image?.image_url?.url;
    if(image?.type!=='image_url'||typeof url!=='string'||url.length>12*1024*1024)throw new Error('Invalid recovery image.');
    const match=/^data:(image\/(?:png|jpeg|gif|webp));base64,([\s\S]*)$/u.exec(url);
    if(!match)throw new Error('Invalid recovery image URL.');
    const encoded=match[2];
    if(encoded.length%4||/[^A-Za-z0-9+/=]/u.test(encoded))throw new Error('Invalid recovery image base64.');
    const bytes=atob(encoded);
    if(bytes.length>8*1024*1024||btoa(bytes)!==encoded)throw new Error('Invalid recovery image bytes.');
    const header=Array.from(bytes.slice(0,12),c=>c.charCodeAt(0));
    const begins=values=>values.every((v,i)=>header[i]===v);
    const mime=begins([137,80,78,71,13,10,26,10])?'image/png':begins([255,216,255])?'image/jpeg':
        /^GIF8[79]a/u.test(bytes)?'image/gif':bytes.startsWith('RIFF')&&bytes.slice(8,12)==='WEBP'?'image/webp':null;
    if(mime!==match[1])throw new Error('Recovery image MIME does not match bytes.');
    return structuredClone(image);
}
export function recoverHistory(history,journal) {
    if(!journal)return history;
    validateHistory(history);
    if(journal.assistant?.role!=='assistant'||!Array.isArray(journal.calls))throw new Error('Invalid recovery journal.');
    const calls=toolCalls(journal.assistant),saved=new Map(),images=[];
    if(!calls.length)throw new Error('Invalid recovery journal batch.');
    for(const item of journal.calls){
        if(!item||!calls.some(call=>call.id===item.id)||saved.has(item.id)||(item.status!==undefined&&!['pending','started','finished'].includes(item.status)))throw new Error('Invalid recovery journal call.');
        saved.set(item.id,item);
    }
    const messages=[...history,structuredClone(journal.assistant)];
    for(const call of calls){
        const item=saved.get(call.id);
        let output='INTERRUPTED: not executed. No previous approval is valid.';
        if(item?.status==='started')output='INTERRUPTED: execution outcome unknown. Inspect workspace files; do not automatically repeat this operation.';
        if(item?.status==='finished'){
            if(typeof item.result?.output!=='string'||(item.result.images!==undefined&&!Array.isArray(item.result.images)))throw new Error('Invalid finished recovery result.');
            output=item.result.output;
            for(const image of item.result.images??[]){
                if(images.length/2>=8)throw new Error('Too many recovery images.');
                images.push({type:'text',text:`Image from tool ${JSON.stringify(call.id)}`},savedImage(image));
            }
        }
        messages.push({role:'tool',tool_call_id:call.id,content:output});
    }
    if(images.length)messages.push({role:'user',content:images});
    validateHistory(messages);return messages;
}

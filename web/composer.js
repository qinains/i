import {uiText} from './i18n.js';
const TEXT_LIMIT=64*1024,IMAGE_LIMIT=8*1024*1024,TOTAL_IMAGES=16*1024*1024;
const byteLength=text=>new TextEncoder().encode(text).length;
const blocks=record=>record.composerBlocks??=[];
export function stageText(record,text){
    text=text.replace(/\r\n?/gu,'\n').replace(/[\x00-\x08\x0b-\x1f\x7f]/gu,'');
    if(byteLength(text)>TEXT_LIMIT)throw new Error('Pasted text exceeds 64 KiB.');
    if([...text].length<1000&&!text.includes('\n'))return text;
    const index=Math.max(0,...blocks(record).filter(b=>b.kind==='text').map(b=>Number(b.token.match(/#(\d+)/u)?.[1]??0)))+1;
    const token=`[Pasted Content #${index} ${[...text].length} chars]`;
    blocks(record).push({kind:'text',token,text});return token;
}
export async function stageImage(record,blob){
    const images=blocks(record).filter(b=>b.kind==='image');
    if(images.length>=8)throw new Error('At most 8 images per message.');
    if(blob.size>IMAGE_LIMIT||images.reduce((n,b)=>n+b.blob.size,blob.size)>TOTAL_IMAGES)throw new Error('Images exceed the size limit (8 MiB each, 16 MiB total).');
    const header=new Uint8Array(await blob.slice(0,12).arrayBuffer());
    const mime=header[0]===137&&header[1]===80&&header[2]===78&&header[3]===71&&header[4]===13&&header[5]===10&&header[6]===26&&header[7]===10?'image/png':
        header[0]===255&&header[1]===216&&header[2]===255?'image/jpeg':
        new TextDecoder().decode(header.slice(0,6)).match(/^GIF8[79]a$/u)?'image/gif':
        new TextDecoder().decode(header.slice(0,4))==='RIFF'&&new TextDecoder().decode(header.slice(8,12))==='WEBP'?'image/webp':null;
    if(!mime||mime!==blob.type)throw new Error('Unsupported image or MIME does not match bytes.');
    // Recheck after the asynchronous header read: another paste may have completed.
    const current=blocks(record).filter(b=>b.kind==='image');
    if(current.length>=8||current.reduce((n,b)=>n+b.blob.size,blob.size)>TOTAL_IMAGES)throw new Error('Too many or oversized images.');
    const token=`[Image #${Math.max(0,...current.map(b=>Number(b.token.match(/#(\d+)/u)?.[1]??0)))+1}]`;
    blocks(record).push({kind:'image',token,blob});return token;
}
export function removeBlock(record,token){record.composerBlocks=blocks(record).filter(b=>b.token!==token);}
export function composeDraft(record,visible){
    const available=record.composerBlocks??[],images=[],seen=new Set();let text='',offset=0;
    // Expand only original input tokens, never token-like strings within pasted data.
    while(offset<visible.length){
        let found=null,position=visible.length;
        for(const block of available){const i=visible.indexOf(block.token,offset);if(i>=0&&i<position){found=block;position=i;}}
        text+=visible.slice(offset,position);if(!found)break;
        if(found.kind==='text')text+=found.text;
        else if(!seen.has(found.token)){seen.add(found.token);images.push(found.blob);}
        offset=position+found.token.length;
        if(byteLength(text)>TEXT_LIMIT)throw new Error('Message text exceeds 64 KiB.');
    }
    if(byteLength(text)>TEXT_LIMIT)throw new Error('Message text exceeds 64 KiB.');
    const hasPastedText=available.some(b=>b.kind==='text'&&visible.includes(b.token));
    return {text:hasPastedText?text:text.trim(),images};
}
export async function imageDataUrls(images){
    return Promise.all(images.map(async blob=>{
        const bytes=new Uint8Array(await blob.arrayBuffer());let binary='';
        for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));
        return `data:${blob.type};base64,${btoa(binary)}`;
    }));
}
export function mountComposer({input,list,getRecord,isDisabled,onChange,onError}){
    const urls=[];let pasteQueue=Promise.resolve();
    function refresh(){
        for(const url of urls)URL.revokeObjectURL(url);urls.length=0;list.replaceChildren();
        const record=getRecord();if(!record)return;
        for(const block of record.composerBlocks??[]){
            if(!input.value.includes(block.token))continue;
            const chip=document.createElement('span');chip.className='composer-chip';chip.dataset.composerToken=block.token;
            if(block.kind==='image'){const img=document.createElement('img');const url=URL.createObjectURL(block.blob);urls.push(url);img.src=url;img.alt=block.token;chip.append(img);}
            const label=document.createElement('span');label.textContent=block.token;chip.append(label);
            const remove=document.createElement('button');remove.type='button';remove.textContent='×';remove.setAttribute('aria-label',uiText('Remove {item}',{item:block.token}));remove.disabled=isDisabled();
            remove.addEventListener('click',()=>{if(isDisabled())return;input.value=input.value.split(block.token).join('');removeBlock(record,block.token);onChange(record,input.value);refresh();input.focus();});
            chip.append(remove);list.append(chip);
        }
        list.hidden=!list.childElementCount;
    }
    function insert(record,token,additions){
        if(getRecord()!==record)throw new Error('Conversation changed; paste again in the intended conversation.');
        if(isDisabled())throw new Error('Wait for the current task before pasting.');
        const start=input.selectionStart??input.value.length,end=input.selectionEnd??start;
        const next=input.value.slice(0,start)+token+input.value.slice(end);
        const nextBlocks=[...(record.composerBlocks??[]),...additions];composeDraft({composerBlocks:nextBlocks},next);
        record.composerBlocks=nextBlocks;
        input.setRangeText(token,start,end,'end');onChange(record,input.value);refresh();
    }
    input.addEventListener('paste',event=>{
        const record=getRecord();if(!record||isDisabled())return;
        const images=[...(event.clipboardData?.items??[])].filter(item=>item.type.startsWith('image/')).map(item=>item.getAsFile()).filter(Boolean);
        const text=event.clipboardData?.getData('text/plain')??'';
        if(!images.length&&!text)return;event.preventDefault();
        const run=async()=>{
            const previous=new Set(record.composerBlocks??[]),staging={composerBlocks:[...previous]},tokens=[];
            try{
                if(images.length){for(const image of images)tokens.push(await stageImage(staging,image));}
                else tokens.push(stageText(staging,text));
                insert(record,tokens.join(' '),staging.composerBlocks.filter(block=>!previous.has(block)));
            }catch(error){onError(error);}
        };pasteQueue=pasteQueue.then(run).catch(onError);
    });
    input.addEventListener('input',()=>{
        const record=getRecord();if(record)record.composerBlocks=(record.composerBlocks??[]).filter(b=>input.value.includes(b.token));refresh();
    });
    return {refresh};
}

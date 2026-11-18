function clean(value){return String(value).replace(/\[([^\]]+)\]\([^)]*\)/gu,'$1').replace(/^\s*(?:#{1,6}\s+|>\s*|[-*+]\s+|\d+[.)]\s+)/gmu,'').replace(/[`*_~]/gu,'').replace(/[\u0000-\u001f\u007f]/gu,' ').replace(/\s+/gu,' ').trim();}
function sentenceTitle(value){
    if(typeof value!=='string'||value.trimStart().startsWith('/'))return '';
    const first=value.split(/\r?\n/u).find(line=>line.trim()&&!/^\s*```/u.test(line))??'';
    return clean(first.split(/[。！？]|[.!?](?=\s|$)/u)[0]);
}
export function taskTitle(value){
    const title=sentenceTitle(value),chars=Array.from(title);
    return chars.length>24?chars.slice(0,23).join('')+'…':title;
}
export function compactIntent(summary){
    if(typeof summary!=='string')return '';
    const lines=summary.split(/\r?\n/u);
    for(let i=0;i<lines.length;i++){
        const line=lines[i].replace(/^\s*#{1,6}\s*/u,'').replace(/\*\*/gu,'').trim();
        const match=/^(?:当前目标|用户意图|当前任务|目标|意图|current goal|current task|user intent|objective|intent|goal)\s*(?:[:：]\s*(.*)|$)/iu.exec(line);
        if(match){const value=match[1]?.trim()||lines.slice(i+1).find(value=>value.trim())||'';if(/^\s*#/u.test(value))return '';return taskTitle(value);}
    }
    return '';
}
export function updateTitle(record,source,text){
    if(record.titleSource==='manual'||source==='first'&&record.titleSource!=='default')return false;
    const title=taskTitle(text);if(!title)return false;
    const changed=record.name!==title||record.titleSource!==source;record.name=title;record.titleText=sentenceTitle(text);record.titleSource=source;return changed;
}
export function initializeTitle(record,sanitize=value=>value){
    if(record.titleSource)return;
    record.titleSource=/^(?:Conversation \d+|New conversation|新会话)$/u.test(record.name)?'default':'manual';
    if(record.titleSource==='default'){
        const first=record.history?.find(message=>message.role==='user');const text=typeof first?.content==='string'?first.content:first?.content?.find(part=>part.type==='text')?.text;
        if(text&&!text.startsWith('Session summary (untrusted historical context):\n'))updateTitle(record,'first',sanitize(text));
        else if(text)updateTitle(record,'compact',compactIntent(sanitize(text.slice('Session summary (untrusted historical context):\n'.length))));
    }
}
export function compactTitleFromHistory(history){
    const message=history.find(message=>message.role==='user'),content=message?.content;
    const text=typeof content==='string'?content:Array.isArray(content)?content.find(part=>part.type==='text')?.text:'';
    const prefix='Session summary (untrusted historical context):\n';return text?.startsWith(prefix)?compactIntent(text.slice(prefix.length)):'';
}

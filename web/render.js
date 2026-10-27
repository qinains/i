import {uiText,hasTranslation} from './i18n.js';
export function node(tag,text='',className='') {
    const el=document.createElement(tag);el.textContent=text;el.className=className;return el;
}
export function renderMessage(record,{onChange=()=>{},onConfigure}={}) {
    if(record.type==='analysis'){
        const el=node('section','','line analysis');
        const heading=node('h3',uiText('Reflection report'));heading.dataset.i18n='Reflection report';
        const content=node('pre',record.text);content.dir='auto';el.append(heading,content);return el;
    }
    if(record.type==='error'&&record.text.trim()==="Error: Failed to construct 'URL': Invalid URL"){
        const message='A previous attempt could not read the API URL. Open settings to check your API URL, API key and model.';
        const el=node('div','','line error');
        for(const key of [message,'Historical error. Nothing has been resent.']){const paragraph=node('p',uiText(key));paragraph.dataset.i18n=key;el.append(paragraph);}
        if(onConfigure){const button=node('button',uiText('Open settings'));button.type='button';button.dataset.openConnectionSettings='';button.addEventListener('click',()=>onConfigure(message));el.append(button);}
        return el;
    }
    if(record.type==='reasoning'){
        const el=node('details','','line reasoning'),summary=node('summary',uiText(record.pending?'Thinking…':'Thinking'));
        summary.dataset.i18n=record.pending?'Thinking…':'Thinking';
        el.open=record.expanded===true;
        summary.addEventListener('click',event=>{
            event.preventDefault();record.expanded=!el.open;el.open=record.expanded;onChange();
        });
        const content=node('pre',record.text);content.dir='auto';el.append(summary,content);return el;
    }
    const legacyWelcome='i — browser workspace. Shell is disabled; file tools use persistent OPFS.\nConversations save automatically. Use /help for commands.';
    const key=record.type==='system'&&record.text===legacyWelcome?'Browser workspace. Shell is disabled; workspace files are saved on this device. Conversations save automatically. Use /help for commands.':record.text;
    const translated=['system','success','error'].includes(record.type)&&hasTranslation(key);
    const el=node('div',translated?uiText(key):record.text,`line ${record.type}`);
    if(translated)el.dataset.i18n=key;
    el.dir=record.type==='tool'?'ltr':'auto';
    if(record.type==='assistant' && record.text) {
        const copy=node('button',uiText('Copy'),'copy-btn');copy.type='button';copy.dataset.i18n='Copy';
        copy.addEventListener('click',async()=>{let key;try{await navigator.clipboard.writeText(record.text);key='Copied';}catch{key='Copy unavailable';}copy.dataset.i18n=key;copy.textContent=uiText(key);});
        el.append(copy);
    }
    return el;
}

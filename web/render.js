import {uiText} from './i18n.js';
export function node(tag,text='',className='') {
    const el=document.createElement(tag);el.textContent=text;el.className=className;return el;
}
export function renderMessage(record,{onChange=()=>{}}={}) {
    if(record.type==='reasoning'){
        const el=node('details','','line reasoning'),summary=node('summary',uiText(record.pending?'Thinking…':'Thinking'));
        el.open=record.expanded===true;
        summary.addEventListener('click',event=>{
            event.preventDefault();record.expanded=!el.open;el.open=record.expanded;onChange();
        });
        el.append(summary,node('pre',record.text));return el;
    }
    const el=node('div',record.text,`line ${record.type}`);
    if(record.type==='assistant' && record.text) {
        const copy=node('button',uiText('Copy'),'copy-btn');copy.type='button';copy.dataset.i18n='Copy';
        copy.addEventListener('click',async()=>{let key;try{await navigator.clipboard.writeText(record.text);key='Copied';}catch{key='Copy unavailable';}copy.dataset.i18n=key;copy.textContent=uiText(key);});
        el.append(copy);
    }
    return el;
}

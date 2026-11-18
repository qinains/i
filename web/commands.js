import {node} from './render.js';
import {uiText} from './i18n.js';
export function commandCandidates(catalog,text,{sessions=[],files=[]}={}){
    if(!text.startsWith('/')||/[\r\n]/u.test(text))return [];
    const commands=catalog.filter(item=>item.platforms.includes('web'));
    if(!/\s/u.test(text))return commands.filter(item=>item.name.startsWith(text)&&item.name!==text).map(item=>({value:item.name,description:item.description}));
    const [name,...args]=text.split(/\s/u),prefix=args.join(' '),command=commands.find(item=>item.name===name);if(!command)return [];
    let candidates=command.arguments.map(value=>({value:`${name} ${value}`,description:command.description}));
    if(name==='/resume')candidates=sessions.map(record=>({value:`/resume ${record.id}`,description:record.name}));
    if(name==='/download')candidates=files.map(file=>({value:`/download ${file.path}`,description:file.path}));
    return candidates.filter(item=>item.value.startsWith(`${name} ${prefix}`)&&item.value!==text).slice(0,40);
}
export function mountCommands(input,catalog,context){
    const menu=node('div','','command-menu');menu.id='command-menu';menu.role='listbox';menu.hidden=true;menu.setAttribute('aria-label',uiText('Available commands'));input.form.prepend(menu);
    input.setAttribute('role','combobox');input.setAttribute('aria-autocomplete','list');input.setAttribute('aria-controls',menu.id);input.setAttribute('aria-expanded','false');
    let choices=[],selected=0,composing=false,dismissed=null;
    function hide(){menu.hidden=true;input.setAttribute('aria-expanded','false');input.removeAttribute('aria-activedescendant');}
    function choose(index){const value=choices[index]?.value;if(!value)return;input.value=value;dismissed=value;hide();input.dispatchEvent(new Event('input',{bubbles:true}));input.focus();}
    function paint(){
        menu.replaceChildren(...choices.map((item,index)=>{const button=node('button');button.type='button';button.role='option';button.id=`command-option-${index}`;button.setAttribute('aria-selected',String(index===selected));button.append(node('strong',item.value),node('span',uiText(item.description)));button.addEventListener('mousedown',event=>event.preventDefault());button.addEventListener('click',()=>choose(index));return button;}));
        menu.hidden=false;input.setAttribute('aria-expanded','true');input.setAttribute('aria-activedescendant',`command-option-${selected}`);menu.children[selected]?.scrollIntoView({block:'nearest'});
    }
    function refresh(){selected=0;if(composing||document.activeElement!==input||dismissed===input.value){hide();return;}choices=commandCandidates(catalog,input.value,context());if(!choices.length){hide();return;}paint();}
    input.addEventListener('input',()=>{if(dismissed!==input.value)dismissed=null;refresh();});input.addEventListener('focus',refresh);input.addEventListener('blur',()=>setTimeout(()=>{if(!menu.contains(document.activeElement))hide();},0));
    input.addEventListener('compositionstart',()=>{composing=true;hide();});input.addEventListener('compositionend',()=>{composing=false;refresh();});
    input.addEventListener('keydown',event=>{
        if(composing||event.isComposing||event.keyCode===229||menu.hidden)return;
        if(event.key==='Escape'){event.preventDefault();event.stopImmediatePropagation();dismissed=input.value;hide();return;}
        if(['ArrowDown','ArrowUp'].includes(event.key)){event.preventDefault();event.stopImmediatePropagation();selected=(selected+(event.key==='ArrowDown'?1:-1)+choices.length)%choices.length;paint();}
        else if(event.key==='Enter'||event.key==='Tab'&&!event.shiftKey){event.preventDefault();event.stopImmediatePropagation();choose(selected);}
    },true);
    return {hide,refresh};
}

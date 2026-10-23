import {browserPath} from './workspaces.js';
import {node} from './render.js';
import {uiText} from './i18n.js';
const MODES=['source','preview','split'];
export function fileKind(path){
    const ext=path.split('.').at(-1).toLowerCase();
    if(['html','htm'].includes(ext))return 'html';
    if(['md','markdown'].includes(ext))return 'markdown';
    if(['png','jpg','jpeg','gif','webp','svg','ico','avif'].includes(ext))return 'image';
    if(!path.split('/').at(-1).includes('.')||['txt','json','jsonl','js','mjs','cjs','ts','tsx','jsx','css','scss','rs','py','sh','toml','yaml','yml','xml','csv','log','sql','c','h','cpp','go','java','lock','ini','env'].includes(ext))return 'text';
    return 'binary';
}
function validPath(value){try{return typeof value==='string'&&value.length<4096&&!!browserPath(value);}catch{return false;}}
export function viewState(value={}){
    const expanded=Array.isArray(value?.expanded)?value.expanded.filter(validPath).slice(0,1000):['/workspace'];
    if(value?.expanded?.length&&!expanded.length)expanded.push('/workspace');
    return {mode:MODES.includes(value?.mode)?value.mode:'preview',expanded,selected:validPath(value?.selected)?value.selected:'',open:value?.open===true};
}
export function persistentView(value){
    const state=viewState(value);state.expanded=state.expanded.filter(path=>browserPath(path).mount!=='tmp');
    if(state.selected&&browserPath(state.selected).mount==='tmp'){state.selected='';state.open=false;}
    return state;
}
export function fileTree(files){
    const roots=['workspace','tmp'].map(mount=>({name:'/'+mount,path:'/'+mount,directory:true,children:[]}));
    const entries=new Map(roots.map(root=>[root.path,root]));
    for(const file of files){
        const location=browserPath(file.path),parts=location.path.split('/').filter(Boolean);let parent=roots[location.mount==='tmp'?1:0],prefix=location.mount==='tmp'?'/tmp/':'';
        parts.forEach((name,index)=>{
            const path=prefix+parts.slice(0,index+1).join('/');let entry=entries.get(path);
            if(!entry){entry={name,path,directory:index<parts.length-1||!!file.directory,children:[],size:file.size??0};parent.children.push(entry);entries.set(path,entry);}
            parent=entry;
        });
    }
    function sort(entry){entry.children.sort((a,b)=>Number(b.directory)-Number(a.directory)||a.name.localeCompare(b.name));entry.children.forEach(sort);}
    roots.forEach(sort);return roots;
}
export function fileIcon(kind,expanded=false){
    const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('viewBox','0 0 24 24');svg.setAttribute('aria-hidden','true');svg.classList.add('file-icon');svg.dataset.kind=kind;
    const path=document.createElementNS(svg.namespaceURI,'path');path.setAttribute('d',kind==='folder'?(expanded?'M3 7V4h6l2 3h10v3M2 10h21l-4 10H4z':'M3 5h6l2 3h10v12H3z'):kind==='image'?'M3 3h18v18H3z M3 17l6-6 4 4 3-3 5 5 M15 7h1':'M5 3h9l5 5v13H5z M14 3v6h5 M8 13h8 M8 17h6');
    svg.append(path);return svg;
}
export function renderFileTree(host,files,state,{toggle,select,temporary=false}){
    host.replaceChildren();host.setAttribute('aria-label',uiText('Workspace files'));
    function item(entry){
        if(entry.directory){
            const details=node('details');details.dataset.path=entry.path;details.open=state.expanded.includes(entry.path);
            const summary=node('summary');summary.title=entry.path;let icon=fileIcon('folder',details.open);summary.append(icon,node('span',entry.name));
            if(['/workspace','/tmp'].includes(entry.path)){
                const count=files.filter(file=>!file.directory&&browserPath(file.path).mount===(entry.path==='/tmp'?'tmp':'workspace')).length;
                details.dataset.fileCount=count;const volatile=temporary||entry.path==='/tmp';
                const label=node('small',uiText(volatile?'{count} · Temporary files':'{count} · Kept after reload',{count}),'note');
                summary.title=uiText(volatile?'Only temporary files are lost on reload. Download them first.':'Files here are saved on this device and remain after reload.');summary.append(label);
            }
            const children=node('div','','tree-children');children.append(...entry.children.map(item));if(!entry.children.length&&files.length)children.append(node('p',uiText('Empty folder'),'note tree-empty'));
            details.append(summary,children);summary.addEventListener('click',event=>{event.preventDefault();details.open=!details.open;const next=fileIcon('folder',details.open);icon.replaceWith(next);icon=next;toggle(entry.path,details.open);});return details;
        }
        const button=node('button','','tree-file');button.type='button';button.dataset.file=entry.path;button.title=entry.path;button.setAttribute('aria-current',String(state.selected===entry.path));button.append(fileIcon(fileKind(entry.path)),node('span',entry.name));button.addEventListener('click',()=>select(entry.path));return button;
    }
    host.append(...fileTree(files).map(item));
    if(!files.length)host.append(node('p',uiText('No files in workspace'),'note'));
}

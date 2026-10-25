import {opfsOpenWorkspace,opfsList,opfsMetadata,opfsRead,opfsWithTransaction} from './platform.js';
import {browserPath} from './workspaces.js';
import {node} from './render.js';
import {withWorkspaceLock} from './state/store.js';
import {uiText} from './i18n.js';
import {fileKind} from './file-browser.js';
import {markdownHtml,staticDocument,textDocument} from './file-content.js';
import {moduleGraph} from './preview-modules.js';
import {highlightSource} from './highlight.js';
import {bindResponsiveDisclosure} from './responsive-ui.js';
import {installPreviewStorage} from './preview-storage.js';
const ORIGIN='https://workspace.invalid';
const SNAPSHOT_LIMIT=16*1024*1024,OUTPUT_LIMIT=48*1024*1024;
const types={html:'text/html',htm:'text/html',css:'text/css',js:'text/javascript',mjs:'text/javascript',json:'application/json',txt:'text/plain',svg:'image/svg+xml',png:'image/png',jpg:'image/jpeg',jpeg:'image/jpeg',gif:'image/gif',webp:'image/webp',ico:'image/x-icon',woff:'font/woff',woff2:'font/woff2',mp3:'audio/mpeg',mp4:'video/mp4'};
function mime(path){const ext=path.split('.').at(-1).toLowerCase();return ext==='avif'?'image/avif':types[ext]??'application/octet-stream';}
function base64(bytes){let text='';for(let i=0;i<bytes.length;i+=8192)text+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(text);}
function dataUrl(type,text){return `data:${type};base64,${base64(new TextEncoder().encode(text))}`;}
export async function previewSnapshot(workspaceId,entry){
    return opfsWithTransaction(await opfsOpenWorkspace(workspaceId),'shared',root=>readSnapshot(root,entry));
}
async function readSnapshot(root,entry){
    const location=browserPath(entry);if(!/\.html?$/iu.test(location.path))throw new Error('Choose an HTML entry file.');
    const prefix=location.mount==='tmp'?'/tmp/':'';
    const files={};const stack=[prefix];let total=0,count=0;
    while(stack.length){
        for(const path of await opfsList(root,stack.pop())){
            if(++count>1000)throw new Error('Preview exceeds 1000 entries.');
            const metadata=await opfsMetadata(root,path);if(!metadata)throw new Error('Preview files changed; refresh again.');
            if(metadata.is_dir){stack.push(path);continue;}
            if(total+metadata.len>SNAPSHOT_LIMIT)throw new Error('Preview snapshot exceeds 16 MiB.');
            const bytes=await opfsRead(root,path,8*1024*1024);total+=bytes.length;
            if(total>SNAPSHOT_LIMIT)throw new Error('Preview snapshot exceeds 16 MiB.');
            files['/'+(prefix?path.slice(prefix.length):path)]={type:mime(path),bytes};
        }
    }
    if(!Object.hasOwn(files,'/'+location.path))throw new Error('HTML entry file not found.');
    return {files,entry:'/'+location.path};
}
// Runs only inside the opaque iframe. All responses come from its own snapshot.
function virtualServer(files,entry){
    const origin='https://workspace.invalid',base=origin+entry;
    window.fetch=async(input,options={})=>{
        const signal=options.signal??input?.signal;if(signal?.aborted)throw new DOMException('Aborted','AbortError');
        const method=String(options.method??input?.method??'GET').toUpperCase();
        if(!['GET','HEAD'].includes(method))throw new TypeError('Preview is read-only; only GET/HEAD are supported.');
        const url=new URL(typeof input==='string'||input instanceof URL?input:input.url,base);
        if(url.origin!==origin||url.username||url.password)throw new TypeError('External network is disabled in preview.');
        const file=files[decodeURIComponent(url.pathname)];
        if(!file)return new Response('Not found',{status:404,headers:{'content-type':'text/plain'}});
        const bytes=Uint8Array.from(atob(file.base64),value=>value.charCodeAt(0));
        const response=new Response(method==='HEAD'?null:bytes,{status:200,headers:{'content-type':file.type}});
        Object.defineProperty(response,'url',{value:url.href});return response;
    };
}
export async function compilePreview({files,entry},{scripts=false}={}){
    const document=new DOMParser().parseFromString(new TextDecoder('utf-8',{fatal:true}).decode(files[entry].bytes),'text/html');
    if(!scripts){for(const element of document.querySelectorAll('script'))element.remove();for(const element of document.querySelectorAll('*'))for(const attribute of [...element.attributes])if(attribute.name.toLowerCase().startsWith('on'))element.removeAttribute(attribute.name);}
    for(const element of document.querySelectorAll('link[rel="modulepreload"]'))element.remove();
    if(document.querySelector('script[type="importmap"]'))throw new Error(uiText('Custom import maps are not supported. Use local relative module paths.'));
    const modules=[...document.querySelectorAll('script[type="module"]')];
    if(modules.length&&!HTMLScriptElement.supports?.('importmap'))throw new Error(uiText('This browser does not support import maps. Update your browser or use a bundled build.'));
    const graph=modules.length?await moduleGraph(files,modules.map(element=>element.hasAttribute('src')?{src:element.getAttribute('src')}:{text:element.textContent}),entry,uiText):null;
    for(const [index,element]of modules.entries()){element.removeAttribute('integrity');element.textContent='';element.src=graph.scripts[index];}
    const base=ORIGIN+entry,cache=new Map();let expanded=0;
    const charge=text=>{expanded+=text.length;if(expanded>OUTPUT_LIMIT)throw new Error('Expanded preview exceeds 48 MiB.');return text;};
    function local(value,relative){
        if(value.startsWith('data:'))return {data:value};
        const url=new URL(value,relative);
        if(url.origin!==ORIGIN||url.username||url.password)throw new Error('External preview resources are disabled. Use local files.');
        let path;try{path=decodeURIComponent(url.pathname);}catch{throw new Error('Invalid resource path.');}
        if(/[\\\0]/u.test(path)||!Object.hasOwn(files,path))throw new Error(`Preview resource not found: ${path}`);
        return {path,fragment:url.hash};
    }
    function asset(value,relative,depth=0){
        const result=local(value,relative);if(result.data)return result.data;
        if(depth>16)throw new Error('CSS import depth exceeds 16; check for cycles.');
        const {path,fragment}=result,file=files[path];let url=cache.get(path);
        if(!url){url=file.type==='text/css'?dataUrl(file.type,css(new TextDecoder().decode(file.bytes),ORIGIN+path,depth+1)):`data:${file.type};base64,${base64(file.bytes)}`;cache.set(path,url);}
        return url+fragment;
    }
    function css(text,relative,depth=0){
        // Escaped URL syntax is intentionally rejected instead of mis-resolving it.
        if(/(?:url\s*\(|@import)[^;{}]*\\/iu.test(text))throw new Error('Escaped CSS resource URLs are not supported in snapshot preview.');
        return text.replace(/@import\s+(["'])([^"']+)\1/giu,(_match,_quote,value)=>`@import url("${charge(asset(value,relative,depth))}")`)
            .replace(/url\(\s*(?:"([^"\r\n]*)"|'([^'\r\n]*)'|([^\s)'"\r\n]+))\s*\)/giu,(match,a,b,c)=>{
                const value=a??b??c;if(value.startsWith('#'))return match;
                return `url("${charge(asset(value,relative,depth))}")`;
            });
    }
    for(const element of document.querySelectorAll('base,meta[http-equiv],iframe,object,embed'))element.remove();
    for(const element of document.querySelectorAll('[srcset]'))element.removeAttribute('srcset');
    for(const element of document.querySelectorAll('script[src]:not([type="module"]),img[src],source[src],audio[src],video[src],input[type="image"][src]'))element.setAttribute('src',charge(asset(element.getAttribute('src'),base)));
    for(const element of document.querySelectorAll('[poster]'))element.setAttribute('poster',charge(asset(element.getAttribute('poster'),base)));
    for(const element of document.querySelectorAll('link[href]')){
        if(['stylesheet','icon'].includes(element.rel))element.href=charge(asset(element.getAttribute('href'),base));else element.remove();
    }
    for(const element of document.querySelectorAll('style'))element.textContent=css(element.textContent,base);
    for(const element of document.querySelectorAll('[style]'))element.setAttribute('style',css(element.getAttribute('style'),base));
    for(const element of document.querySelectorAll('a[href],area[href]'))if(!element.getAttribute('href').startsWith('#')){element.removeAttribute('href');element.title='Open other HTML files using the preview entry control.';}
    const policy=document.createElement('meta');policy.httpEquiv='Content-Security-Policy';policy.content="default-src 'none'; script-src 'unsafe-inline' data:; style-src 'unsafe-inline' data:; img-src data:; media-src data:; font-src data:; connect-src 'none'; frame-src 'none'; worker-src 'none'; object-src 'none'; form-action 'none'; base-uri about:";
    // Resources and module URLs are already rewritten; fragments must stay in srcdoc.
    const baseElement=document.createElement('base');baseElement.href='about:srcdoc';
    document.head.prepend(policy,baseElement);
    if(graph){const map=document.createElement('script');map.type='importmap';map.textContent=JSON.stringify({imports:graph.imports}).replaceAll('<','\\u003c');baseElement.after(map);}
    if(scripts){const encoded=Object.fromEntries(Object.entries(files).map(([path,file])=>[path,{type:file.type,base64:base64(file.bytes)}]));
        const bootstrap=document.createElement('script');bootstrap.src=dataUrl('text/javascript',`(${installPreviewStorage.toString()})();(${virtualServer.toString()})(${JSON.stringify(encoded)},${JSON.stringify(entry)});`);baseElement.after(bootstrap);}
    const html='<!doctype html>'+document.documentElement.outerHTML;
    if(new TextEncoder().encode(html).length>OUTPUT_LIMIT)throw new Error('Expanded preview exceeds 48 MiB.');
    return html;
}
export function mountPreview({owner,isBusy,onError,onChange=()=>{},onBack=()=>{}}){
    function iconButton(id,label,path){
        const button=node('button','','icon-action');button.id=id;button.type='button';
        button.setAttribute('aria-label',uiText(label));button.title=uiText(label);button.dataset.i18nAriaLabel=label;button.dataset.i18nTitle=label;
        const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('viewBox','0 0 24 24');svg.setAttribute('aria-hidden','true');
        const shape=document.createElementNS(svg.namespaceURI,'path');shape.setAttribute('d',path);svg.append(shape);button.append(svg);return button;
    }
    const panel=node('section');panel.id='preview-panel';panel.hidden=true;panel.setAttribute('aria-label','Static HTML preview');
    const tools=node('div','','preview-toolbar'),entry=node('input');entry.id='preview-entry';entry.value='index.html';entry.setAttribute('aria-label','HTML entry path');
    const refresh=iconButton('refresh-preview','Refresh preview','M20 10a8 8 0 1 0-1 8M20 4v6h-6');
    const close=iconButton('close-preview','Close preview','m6 6 12 12M6 18 18 6');
    const fullscreen=iconButton('preview-fullscreen','Toggle fullscreen','M9 4H4v5m11-5h5v5M4 15v5h5m11-5v5h-5');
    const exitFullscreen=iconButton('preview-exit-fullscreen','Exit fullscreen','M4 9h5V4m6 0v5h5M4 15h5v5m6 0v-5h5');
    const status=node('p','','note');status.id='preview-status';status.role='status';const frame=node('iframe');frame.id='preview-frame';frame.title='Isolated HTML preview';frame.setAttribute('sandbox','');frame.referrerPolicy='no-referrer';frame.hidden=true;
    const back=node('button','Files');back.id='preview-back';back.dataset.i18n='Files';
    const mode=node('select');mode.id='preview-mode';mode.setAttribute('aria-label',uiText('Preview mode'));mode.dataset.i18nAriaLabel='Preview mode';
    for(const [value,text]of [['source','Source'],['preview','Preview'],['split','Split']]){const option=node('option',text);option.value=value;option.dataset.i18n=text;mode.append(option);}mode.value='preview';
    const source=node('pre');source.id='preview-source';source.tabIndex=0;source.setAttribute('aria-label',uiText('Source'));source.dataset.i18nAriaLabel='Source';source.hidden=true;
    const content=node('div','','preview-content');content.append(source,frame);
    const trust=node('label','','preview-scripts-label');const caption=node('span','允许JS');caption.dataset.i18n=caption.textContent;const scripts=node('input');scripts.id='preview-scripts';scripts.type='checkbox';scripts.checked=false;trust.append(scripts,caption);
    const actions=node('div','','preview-actions');actions.append(mode,refresh,fullscreen,close);
    const options=node('details');options.id='preview-options';options.open=true;
    const optionToggle=node('summary','','icon-action');optionToggle.setAttribute('aria-label',uiText('Preview options'));optionToggle.dataset.i18nAriaLabel='Preview options';optionToggle.title=uiText('Preview options');optionToggle.dataset.i18nTitle='Preview options';
    const toggleIcon=document.createElementNS('http://www.w3.org/2000/svg','svg');toggleIcon.setAttribute('viewBox','0 0 24 24');toggleIcon.setAttribute('aria-hidden','true');
    const togglePath=document.createElementNS(toggleIcon.namespaceURI,'path');togglePath.setAttribute('d','M4 7h16M4 17h16M8 4v6m8 4v6');toggleIcon.append(togglePath);optionToggle.append(toggleIcon);
    const optionBody=node('div','','preview-options'),extraControls=node('div','','preview-extra-controls');optionBody.append(extraControls,trust,status);options.append(optionToggle,optionBody);
    tools.append(back,entry,actions);panel.append(tools,options,content,exitFullscreen);document.getElementById('main-content').append(panel);
    bindResponsiveDisclosure(options,{onLayout:mobile=>{if(mobile)extraControls.append(mode,fullscreen);else{actions.prepend(mode);actions.insertBefore(fullscreen,close);}}});
    const button=node('button','Preview HTML');button.id='btn-preview';document.getElementById('btn-download').before(button);
    button.dataset.i18n=button.textContent;
    entry.dataset.i18nAriaLabel='HTML entry path';
    let generation=0,workspaceId,previousOptionsOpen=false;
    function setFullscreen(enabled,{focus=true}={}){
        if(enabled===panel.classList.contains('fullscreen'))return;
        if(enabled){previousOptionsOpen=options.open;options.open=false;}
        panel.classList.toggle('fullscreen',enabled);
        if(!enabled)options.open=previousOptionsOpen;
        fullscreen.setAttribute('aria-pressed',String(enabled));
        if(focus)(enabled?exitFullscreen:fullscreen).focus();
    }
    fullscreen.setAttribute('aria-pressed','false');
    async function show(path){
        if(isBusy())throw new Error('Finish the active task before taking a preview snapshot.');
        if(path)entry.value=path;const current=owner();if(!current)return;
        const id=++generation,selectedEntry=entry.value,kind=fileKind(selectedEntry),selectedMode=mode.value,execute=kind==='html'&&scripts.checked;workspaceId=current.workspaceId;panel.hidden=false;frame.hidden=true;source.hidden=true;frame.removeAttribute('srcdoc');source.textContent='';refresh.disabled=true;scripts.disabled=true;entry.disabled=true;mode.disabled=true;trust.hidden=kind!=='html'||selectedMode==='source';status.textContent=uiText('Building snapshot…');content.dataset.mode=selectedMode;
        onChange({selected:selectedEntry,mode:selectedMode,open:true});
        try{
            const result=await withWorkspaceLock(current.workspaceId,async()=>{
                browserPath(selectedEntry);return opfsWithTransaction(await opfsOpenWorkspace(current.workspaceId),'shared',async root=>{
                    const bytes=await opfsRead(root,selectedEntry,8*1024*1024);
                    const textual=!['image','binary'].includes(kind);let text=textual?new TextDecoder('utf-8',{fatal:true}).decode(bytes):uiText('Binary file; source view is unavailable.');
                    if(selectedMode==='source')return {text};
                    if(kind==='html')return {text,snapshot:await readSnapshot(root,selectedEntry)};
                    if(kind==='markdown')return {text,html:staticDocument(markdownHtml(text))};
                    if(kind==='image')return {text,html:staticDocument(`<img alt="" src="data:${mime(selectedEntry)};base64,${base64(bytes)}">`)};
                    if(kind==='binary')return {text};
                    let rendered=text;if(/\.json$/iu.test(selectedEntry))try{rendered=JSON.stringify(JSON.parse(text),null,2);}catch{}
                    return {text,html:textDocument(rendered)};
                });
            });
            if(id!==generation)return;
            // Rendering uses immutable snapshot bytes, not live OPFS handles.
            if(result.snapshot){result.html=await compilePreview(result.snapshot,{scripts:execute});delete result.snapshot;}
            if(id!==generation)return;
            const ext=selectedEntry.split('.').at(-1)?.toLowerCase()||'';
            source.hidden=selectedMode==='preview'&&!!result.html;
            if(!source.hidden){
                if(result.text.length>128*1024)source.textContent=result.text;
                else source.innerHTML=highlightSource(result.text,ext);
            }
            frame.setAttribute('sandbox',execute&&selectedMode!=='source'?'allow-scripts':'');
            if(result.html){frame.srcdoc=result.html;frame.hidden=selectedMode==='source';}
            status.textContent=uiText(kind==='html'&&selectedMode!=='source'?(execute?'Read-only snapshot. Trusted JS, local ES modules and fetch enabled. No backend or CDN. CSP is not a complete network sandbox.':'Read-only snapshot. Scripts disabled. Local HTML / CSS / images supported; no backend.'):'Read-only file preview.');
            if(execute&&selectedMode!=='source')status.textContent+=' '+uiText('Preview storage is temporary and isolated; refreshing the preview clears it.');
        }
        catch(error){if(id===generation){
            options.open=true;
            if(error?.name==='NotFoundError'){
                const location=browserPath(selectedEntry),path=`/${location.mount}/${location.path}`;
                status.textContent=uiText('Preview file not found: {path}. It may not have been created or may have been moved. Select an existing file; no file was created or deleted.',{path});
                onChange({open:false});
            }else{status.textContent=String(error.message??error);onError(error);}
        }}
        finally{if(id===generation){refresh.disabled=isBusy();scripts.disabled=isBusy();entry.disabled=isBusy();mode.disabled=isBusy();}}
    }
    function hide(notify=true){setFullscreen(false,{focus:false});++generation;workspaceId=null;panel.hidden=true;frame.hidden=true;source.hidden=true;source.textContent='';frame.removeAttribute('srcdoc');refresh.disabled=false;scripts.disabled=false;entry.disabled=false;mode.disabled=false;if(notify)onChange({open:false});}
    scripts.addEventListener('change',()=>{
        if(scripts.checked&&!confirm(uiText('Run trusted JavaScript? Scripts can read ALL files in this preview snapshot. The opaque frame protects Harness storage, but browser CSP is not complete network isolation. Only enable for code you trust.'))){scripts.checked=false;return;}
        show().catch(onError);
    });
    mode.addEventListener('change',()=>{show().catch(onError);});back.addEventListener('click',()=>{hide();onBack();});
    button.addEventListener('click',()=>show().catch(onError));refresh.addEventListener('click',()=>show().catch(onError));close.addEventListener('click',()=>hide());
    fullscreen.addEventListener('click',()=>setFullscreen(!panel.classList.contains('fullscreen')));
    exitFullscreen.addEventListener('click',()=>setFullscreen(false));
    document.addEventListener('keydown',event=>{if(event.key==='Escape'&&!event.defaultPrevented&&panel.classList.contains('fullscreen')){event.preventDefault();setFullscreen(false);}});
    return {show,hide,restore:async state=>{hide(false);mode.value=state.mode;if(state.selected)entry.value=state.selected;else entry.value='index.html';if(state.open&&state.selected)await show(state.selected);},selectWorkspace:id=>{if(workspaceId&&workspaceId!==id)hide(false);}};
}

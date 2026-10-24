import {node} from '../render.js';
import {uiText} from '../i18n.js';
import {isTemporaryWorkspace} from '../workspaces.js';
import {promptEditor} from './prompts.js';
const text=(tag,key,className='')=>{const element=node(tag,uiText(key),className);element.dataset.i18n=key;return element;};
function permissions(p){return [uiText(p.workspace==='read-write'?'Read and modify workspace files':p.workspace==='read'?'Read workspace files':'No workspace file access'),uiText(p.storage?'Private plugin storage':'No private plugin storage')].join(' · ');}
export function mountPluginSettings({manager,currentWorkspace,isBusy,onError,onBusy=()=>{},promptStore,defaultPrompt}){
    const panel=document.querySelector('[data-settings-panel=plugins]'),actions=node('div','','actions');let busy=false,preview;
    const file=node('input');file.id='plugin-file';file.type='file';file.accept='.hplugin.zip,application/zip';file.setAttribute('aria-label',uiText('Choose plugin package'));file.dataset.i18nAriaLabel='Choose plugin package';
    const status=node('p','','note');status.id='plugin-status';status.role='status';
    const previewBox=node('section','','plugin-card');previewBox.id='plugin-preview';previewBox.hidden=true;
    const list=node('div');list.id='plugin-list';
    panel.append(text('h2','Installed plugins'),text('p','Import a Rust-built WASM plugin package. Installation does not enable it.','note'),file,status,previewBox,list);
    const getManager=()=>typeof manager==='function'?manager():Promise.resolve(manager);
    function controls(){for(const element of panel.querySelectorAll('input,button'))element.disabled=busy||isBusy();}
    async function action(fn){if(busy||isBusy())return;busy=true;onBusy(true);controls();try{await fn();}catch(error){status.textContent=uiText('Plugin operation failed: {reason}',{reason:String(error?.message??error)});onError(error);}finally{busy=false;onBusy(false);controls();}}
    function button(key,fn){const element=text('button',key);element.type='button';element.addEventListener('click',()=>action(fn));return element;}
    async function refresh(){
        const workspace=currentWorkspace();if(!workspace)return;
        const m=await getManager(),records=await m.list();list.replaceChildren();
        if(!isTemporaryWorkspace(workspace)&&promptStore&&records.some(p=>p.id==='builtin.prompts'&&p.status==='installed'))list.append(await promptEditor({store:promptStore(),workspaceId:workspace,defaultPrompt:await defaultPrompt(),isBusy}));
        if(!records.length)list.append(text('p','No plugins installed.','note'));
        for(const record of records){
            const card=node('section','','plugin-card');card.dataset.pluginId=record.id;
            card.append(node('h3',`${record.manifest.name} · ${record.version}`),node('p',record.id,'note'),node('p',record.digest,'plugin-digest note'));
            const grant=record.grants.find(g=>g.workspaceId===workspace&&g.version===record.version&&g.digest===record.digest),available=record.status==='installed';
            card.append(text('p',!available?'Plugin files unavailable or damaged':grant?'Enabled':'Disabled','plugin-state'));
            if(available){
                card.append(node('p',permissions(record.manifest.permissions),'note'));
                const tools=node('ul');for(const tool of record.manifest.tools)tools.append(node('li',`${tool.name} — ${tool.description}`));card.append(tools);
            }
            const actions=node('div','','actions');
            if(available&&!isTemporaryWorkspace(workspace)){
                const toggle=button(grant?'Disable':'Enable for this workspace',async()=>{
                    if(currentWorkspace()!==workspace)throw new Error('Workspace changed. Reopen plugin settings.');
                    if(grant)await m.disable(workspace,record.id);
                    else{if(!confirm(uiText('Enable this plugin for the current workspace with these permissions?')+'\n'+permissions(record.manifest.permissions)))return;await m.enable(workspace,record.id,record.manifest.permissions);}
                    await refresh();
                });toggle.dataset.pluginAction=grant?'disable':'enable';actions.append(toggle);
                const clear=button('Clear private data…',async()=>{if(confirm(uiText('Clear this plugin’s private data in the current workspace? Project files are retained.'))){await m.clearData(workspace,record.id);status.textContent=uiText('Private plugin data cleared.');}});actions.append(clear);
            }else if(isTemporaryWorkspace(workspace))card.append(text('p','Plugins are unavailable in temporary workspaces.','note'));
            const uninstall=button('Uninstall…',async()=>{if(!confirm(uiText('Uninstall this plugin? Project files and private plugin data are retained.')))return;await m.uninstall(record.id);await refresh();});uninstall.dataset.pluginAction='uninstall';actions.append(uninstall);card.append(actions);list.append(card);
        }
        controls();
    }
    file.addEventListener('change',()=>action(async()=>{
        preview=null;previewBox.replaceChildren();previewBox.hidden=true;const selected=file.files[0];if(!selected)return;
        if(selected.size>10*1024*1024)throw new Error(uiText('Plugin package exceeds 10 MiB.'));
        const m=await getManager();preview=await m.preview(new Uint8Array(await selected.arrayBuffer()));
        const p=preview;previewBox.append(node('h3',`${p.manifest.name} · ${p.manifest.version}`),text('p','Unverified publisher','plugin-warning'),node('p',p.manifest.id,'note'),node('p',permissions(p.manifest.permissions),'note'),node('p',p.digest,'plugin-digest note'));
        const tools=node('ul');for(const tool of p.manifest.tools)tools.append(node('li',`${tool.name} — ${tool.description}`));previewBox.append(tools);
        const install=button('Install / update',async()=>{await m.install(p);preview=null;previewBox.hidden=true;file.value='';status.textContent=uiText('Installed. Enable it for a workspace. Updates require re-enabling.');await refresh();});install.id='plugin-install';
        const cancel=button('Cancel',async()=>{preview=null;previewBox.hidden=true;previewBox.replaceChildren();file.value='';});previewBox.append(install,cancel);previewBox.hidden=false;
    }));
    return {refresh:()=>action(refresh),updateControls:controls,dispose:()=>{preview=null;panel.replaceChildren();}};
}

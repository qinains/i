import {withWorkspaceLock} from '../state/store.js';
import {node} from '../render.js';
import {uiText} from '../i18n.js';
export const PROMPT_PLUGIN='builtin.prompts';
export const promptOwner=workspaceId=>({workspaceId,pluginId:PROMPT_PLUGIN});
export function validatePrompt(value){if(typeof value!=='string'||!value.trim()||new TextEncoder().encode(value).length>131072)throw new Error('Prompt must contain 1-131072 UTF-8 bytes.');return value;}
export async function ensureDefaultPrompts(manager,store,workspaceId){
    if(!workspaceId||workspaceId.startsWith('temp-'))return;
    const owner=promptOwner(workspaceId);if(await store.defaultInitialized(owner))return;
    const installed=(await manager.list()).find(p=>p.id===PROMPT_PLUGIN);
    const response=await fetch(new URL('../plugin-pkg/builtin.prompts-1.0.0.hplugin.zip',import.meta.url),{credentials:'omit'});
    if(!response.ok)throw new Error('Default prompt plugin could not be loaded.');
    const preview=await manager.preview(new Uint8Array(await response.arrayBuffer()));
    if(preview.manifest.id!==PROMPT_PLUGIN||preview.manifest.permissions.workspace!=='none')throw new Error('Invalid default prompt plugin.');
    if(installed&&(installed.status!=='installed'||installed.digest!==preview.digest))throw new Error('Default prompt plugin identity differs from the bundled package.');
    if(!installed)await manager.install(preview);
    await manager.enable(workspaceId,PROMPT_PLUGIN,{workspace:'none',storage:true});
    await store.markDefaultInitialized(owner);
}
export async function applyPrompts(agent,snapshot,store){
    const enabled=snapshot.tools.some(tool=>tool.pluginId===PROMPT_PLUGIN);
    const saved=enabled?await store.kvGet(promptOwner(snapshot.workspaceId),'system'):null;
    agent.set_system_prompt(saved===null?'':validatePrompt(saved));
}
export async function promptEditor({store,workspaceId,defaultPrompt,isBusy,onChange}){
    const owner=promptOwner(workspaceId);let baseline=await store.kvGet(owner,'system');
    const card=node('section','','plugin-card'),title=node('h3',uiText('Prompt manager')),input=node('textarea');input.id='prompt-system';input.rows=12;input.value=baseline??defaultPrompt;input.setAttribute('aria-label',uiText('System prompt'));
    const note=node('p',uiText('Changes apply to the next task in this workspace. AI changes always require approval. API credentials are not accessible.'),'note');
    const status=node('p','','note');status.id='prompt-status';status.role='status';const buttons=node('div','','actions');
    for(const [id,label,reset]of [['save-prompts','Save prompts',false],['reset-prompts','Reset prompt',true]]){
        const button=node('button',uiText(label));button.id=id;button.type='button';
        button.addEventListener('click',async()=>{
            if(isBusy())return;
            try{await withWorkspaceLock(workspaceId,async()=>{
                const value=reset?null:validatePrompt(input.value);
                if(await store.kvGet(owner,'system')!==baseline)throw new Error('Prompt changed in another window. Reopen settings before saving.');
                if(reset)await store.kvDelete(owner,'system');else await store.kvPut(owner,'system',value);
                baseline=value;input.value=value??defaultPrompt;status.textContent=uiText('Saved. Applies to the next task.');
            });onChange?.();}catch(error){status.textContent=uiText(error.message);}
        });buttons.append(button);
    }
    card.append(title,note,input,buttons,status);return card;
}

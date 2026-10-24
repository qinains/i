import {opfsOpenWorkspace,opfsRead,opfsWrite,opfsList,opfsMetadata,opfsWithTransaction} from '../platform.js';
import {browserPath} from '../workspaces.js';
import {persistentWorkspace} from './store.js';
import {PROMPT_PLUGIN,validatePrompt} from './prompts.js';
const encoder=new TextEncoder(),LIMIT=8*1024*1024;
function args(value,keys){if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!keys.includes(k))||keys.some(k=>!Object.hasOwn(value,k)))throw new Error('Invalid plugin host operation arguments.');}
export function createPluginBroker({snapshot,descriptor,context,store}){
    persistentWorkspace(context.workspaceId);
    if(snapshot.workspaceId!==context.workspaceId)throw new Error('Plugin workspace binding mismatch.');
    const granted=snapshot.tools.find(t=>t.publicName===descriptor.publicName);
    if(!granted||granted.pluginId!==descriptor.pluginId||granted.version!==descriptor.version||granted.digest!==descriptor.digest)throw new Error('Plugin is not authorized in this task.');
    const permission=granted.permissions,owner={workspaceId:context.workspaceId,pluginId:granted.pluginId};let calls=0;
    const check=()=>{context.signal?.throwIfAborted();if(context.isActive&&!context.isActive())throw new Error('Plugin invocation is no longer active.');};
    return async request=>{
        const id=request?.id;
        try{
            check();if(id!==++calls||calls>128)throw new Error('Plugin host operation sequence/limit exceeded.');
            const {op,args:value}=request;let result;
            if(['fs.read','fs.write','fs.list'].includes(op)){
                if(!['read','read-write'].includes(permission.workspace)||op==='fs.write'&&permission.workspace!=='read-write')throw new Error('Plugin file permission denied.');
                args(value,op==='fs.write'?['path','text']:['path']);browserPath(value.path);
                const root=await opfsOpenWorkspace(context.workspaceId);check();
                if(op==='fs.read')result=await opfsWithTransaction(root,'shared',async lease=>{check();return {text:new TextDecoder('utf-8',{fatal:true}).decode(await opfsRead(lease,value.path,LIMIT))};},context.signal);
                if(op==='fs.list'){
                    result=await opfsWithTransaction(root,'shared',async lease=>{
                        const paths=await opfsList(lease,value.path);if(paths.length>1000)throw new Error('Plugin file listing exceeds 1000 entries.');
                        const entries=[];for(const path of paths){check();const m=await opfsMetadata(lease,path);if(!m)throw new Error('Workspace files changed.');entries.push({path,directory:m.is_dir,size:m.len});}return {entries};
                    },context.signal);
                }
                if(op==='fs.write'){
                    if(typeof value.text!=='string')throw new Error('Plugin file text must be a string.');const bytes=encoder.encode(value.text);if(bytes.length>LIMIT)throw new Error('Plugin file exceeds 8 MiB.');
                    if(context.mode!=='auto'){
                        context.pauseBudget(true);let approved;
                        try{approved=await context.approve(`PLUGIN ${granted.pluginId}\nWRITE ${value.path}\n${value.text}`);}finally{context.pauseBudget(false);}
                        if(!approved)throw new Error('Plugin file write denied.');
                    }
                    check();await context.beforeEffect?.();check();
                    await opfsWithTransaction(root,'exclusive',async lease=>{
                        check();if(context.effects)context.effects.started=true;
                        await opfsWrite(lease,value.path,bytes);
                    },context.signal);result={bytes:bytes.length};
                }
            }else if(['kv.get','kv.put','kv.delete'].includes(op)){
                if(!permission.storage)throw new Error('Plugin private storage permission denied.');
                args(value,op==='kv.put'?['key','value']:['key']);
                if(granted.pluginId===PROMPT_PLUGIN){
                    if(value.key!=='system')throw new Error('Unknown prompt.');
                    if(op==='kv.put')validatePrompt(value.value);
                    if(op!=='kv.get'){
                        context.pauseBudget(true);let approved;
                        try{approved=await context.approve(`CHANGE SYSTEM PROMPT\nApplies to the next task.\n${op==='kv.delete'?'Reset to built-in prompt':value.value}`,{force:true});}finally{context.pauseBudget(false);}
                        if(!approved)throw new Error('Prompt change denied.');check();
                    }
                }
                if(op==='kv.get')result={value:await store.kvGet(owner,value.key)};
                else{await context.beforeEffect?.();check();if(context.effects)context.effects.started=true;if(op==='kv.put')await store.kvPut(owner,value.key,value.value);else await store.kvDelete(owner,value.key);result={};}
            }else throw new Error('Unsupported plugin host operation.');
            check();return {id,ok:true,result};
        }catch(error){return {id,ok:false,error:String(error?.message??error).slice(0,4096)};}
    };
}

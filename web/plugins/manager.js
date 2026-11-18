import {pluginId,persistentWorkspace} from './store.js';
const ranks={none:0,read:1,'read-write':2},LIMIT=10*1024*1024;
export async function digest(bytes){return [...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(v=>v.toString(16).padStart(2,'0')).join('');}
export async function publicToolName(id,name){pluginId(id);if(!/^[a-z][a-z0-9_]{0,31}$/u.test(name))throw new Error('Invalid plugin tool name.');return `p_${name}_${(await digest(new TextEncoder().encode(id+'\0'+name))).slice(0,16)}`;}
function validPermissions(p){return !!p&&Object.keys(p).length===2&&Object.hasOwn(ranks,p.workspace)&&typeof p.storage==='boolean';}
export function grantFits(grant,declared){return validPermissions(grant)&&validPermissions(declared)&&ranks[grant.workspace]<=ranks[declared.workspace]&&(!grant.storage||declared.storage);}
function identity(record){pluginId(record.id);if(!/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/u.test(record.version)||record.version.length>32||!/^[a-f0-9]{64}$/u.test(record.digest))throw new Error('Invalid installed plugin identity.');return [record.id,record.version,record.digest];}
function equal(a,b){return a.id===b.id&&a.version===b.version&&a.digest===b.digest;}
async function directory(record,create=false){let dir=await navigator.storage.getDirectory();for(const part of ['harness','plugins',...identity(record)])dir=await dir.getDirectoryHandle(part,{create});return dir;}
function lock(mode,fn){if(!navigator.locks?.request)throw new Error('Plugin locking is unavailable.');return navigator.locks.request('harness:plugins',{mode,ifAvailable:true},held=>{if(!held)throw new Error('Plugins are in use in another window. Stop active tasks and try again.');return fn();});}
function freeze(value){if(value&&typeof value==='object'){for(const child of Object.values(value))freeze(child);Object.freeze(value);}return value;}
export class PluginManager {
    #store;#inspect;#previews=new WeakMap();
    constructor({store,inspectPackage}){this.#store=store;this.#inspect=inspectPackage;}
    async preview(bytes){
        if(!(bytes instanceof Uint8Array)||bytes.length>LIMIT)throw new Error('Plugin package exceeds 10 MiB.');
        const copy=bytes.slice(),verified=await this.#inspect(copy.slice());
        const view={manifest:structuredClone(verified.manifest),digest:verified.digest,license:verified.license};this.#previews.set(view,copy);return view;
    }
    async #read(record){
        identity(record);const dir=await directory(record),file=await (await dir.getFileHandle('package.hplugin.zip')).getFile();
        if(file.size>LIMIT)throw new Error('Installed plugin exceeds package limit.');
        const bytes=new Uint8Array(await file.arrayBuffer());
        if(await digest(bytes)!==record.packageDigest)throw new Error('Installed plugin package digest mismatch.');
        const verified=await this.#inspect(bytes);
        if(!equal(record,{id:verified.manifest.id,version:verified.manifest.version,digest:verified.digest}))throw new Error('Installed plugin identity mismatch.');
        return verified;
    }
    async install(preview){return lock('exclusive',async()=>{
        const original=this.#previews.get(preview);if(!original)throw new Error('Select and inspect a plugin package first.');
        const verified=await this.#inspect(original.slice()),m=verified.manifest;
        const record={id:m.id,version:m.version,digest:verified.digest,packageDigest:await digest(original),manifest:m,status:'installed'};identity(record);
        const state=await this.#store.loadRegistry(),sameVersion=state.packages.find(p=>p.id===record.id&&p.version===record.version);
        if(sameVersion){
            if(!equal(sameVersion,record)||sameVersion.packageDigest!==record.packageDigest)throw new Error('This plugin version already exists with different content. Use a new version.');
            await this.#read(sameVersion);return sameVersion;
        }
        const dir=await directory(record,true),writer=await (await dir.getFileHandle('package.hplugin.zip',{create:true})).createWritable();
        try{await writer.write(original.slice());await writer.close();}catch(error){try{await writer.abort();}catch{}throw error;}
        await this.#read(record);
        await this.#store.commitRegistry({putPackages:[record],putActive:[{id:record.id,version:record.version,digest:record.digest}],deleteGrants:state.grants.filter(g=>g.pluginId===record.id).map(g=>[g.workspaceId,g.pluginId])},state.revision);
        this.#previews.delete(preview);return structuredClone(record);
    });}
    async list(){return lock('shared',async()=>{
        const state=await this.#store.loadRegistry(),items=[];
        for(const active of state.active){
            const record=state.packages.find(p=>equal(p,active));
            try{if(!record)throw new Error('Installed plugin record missing.');const v=await this.#read(record);items.push({...record,manifest:v.manifest,status:'installed',grants:state.grants.filter(g=>g.pluginId===active.id)});}
            catch{items.push({id:String(active.id),version:String(active.version),digest:String(active.digest),manifest:{name:String(active.id)},status:'unavailable',grants:[]});}
        }
        return items;
    });}
    async enable(workspaceId,id,permissions){persistentWorkspace(workspaceId);pluginId(id);return lock('exclusive',async()=>{
        const state=await this.#store.loadRegistry(),active=state.active.find(p=>p.id===id),record=state.packages.find(p=>active&&equal(p,active));
        if(!record)throw new Error('Plugin is not installed.');const verified=await this.#read(record);
        if(!grantFits(permissions,verified.manifest.permissions))throw new Error('Plugin permission grant exceeds its declaration.');
        await this.#store.commitRegistry({putGrants:[{workspaceId,pluginId:id,version:record.version,digest:record.digest,permissions:structuredClone(permissions)}]},state.revision);
    });}
    async disable(workspaceId,id){persistentWorkspace(workspaceId);pluginId(id);return lock('exclusive',async()=>{const s=await this.#store.loadRegistry();await this.#store.commitRegistry({deleteGrants:[[workspaceId,id]]},s.revision);});}
    async uninstall(id,{clearData=false}={}){pluginId(id);return lock('exclusive',async()=>{
        const s=await this.#store.loadRegistry();await this.#store.commitRegistry({deleteActive:[id],deletePackages:s.packages.filter(p=>p.id===id).map(identity),deleteGrants:s.grants.filter(g=>g.pluginId===id).map(g=>[g.workspaceId,g.pluginId])},s.revision);
        if(clearData)await this.#store.clearPluginData(id);
        try{const root=await (await navigator.storage.getDirectory()).getDirectoryHandle('harness');const packages=await root.getDirectoryHandle('plugins');await packages.removeEntry(id,{recursive:true});}catch(error){if(error.name!=='NotFoundError')throw new Error('Plugin disabled and unregistered, but package file cleanup failed.');}
    });}
    async clearData(workspaceId,id){persistentWorkspace(workspaceId);pluginId(id);return lock('exclusive',()=>this.#store.kvClear({workspaceId,pluginId:id}));}
    async loadModule(descriptor){const state=await this.#store.loadRegistry(),record=state.packages.find(p=>equal(p,{id:descriptor.pluginId,...descriptor}));if(!record)throw new Error('Plugin version is unavailable.');return (await this.#read(record)).wasm;}
    async withSnapshot(workspaceId,fn){persistentWorkspace(workspaceId);return lock('shared',async()=>{
        const state=await this.#store.loadRegistry(),tools=[],unavailable=[];
        for(const grant of state.grants.filter(g=>g.workspaceId===workspaceId)){
            try{
                const active=state.active.find(p=>p.id===grant.pluginId),record=state.packages.find(p=>active&&equal(p,active));
                if(!record||record.version!==grant.version||record.digest!==grant.digest)throw new Error('Plugin grant version mismatch.');
                const verified=await this.#read(record),m=verified.manifest;if(!grantFits(grant.permissions,m.permissions))throw new Error('Invalid plugin grant.');
                for(const tool of m.tools){const publicName=await publicToolName(m.id,tool.name);if(tools.some(t=>t.publicName===publicName))throw new Error('Plugin tool name collision.');tools.push({pluginId:m.id,version:m.version,digest:verified.digest,localName:tool.name,publicName,description:tool.description,parameters:tool.parameters,permissions:structuredClone(grant.permissions)});}
                if(tools.length>128)throw new Error('Enabled plugins exceed 128 tools.');
            }catch{unavailable.push(String(grant.pluginId));}
        }
        // Never expose a partially added plugin if any of its tools failed validation.
        const safeTools=tools.filter(t=>!unavailable.includes(t.pluginId));
        return fn(freeze({workspaceId,revision:state.revision,tools:safeTools,unavailable}));
    });}
}

const DB='harness_state';
import {validateSession} from './records.js';
const LIMIT=64*1024*1024;
const MiB=1024*1024;
async function prepareImages(record,budget){
    validateSession(record);
    const draft=record.composerBlocks??[],queued=record.pendingMessages??[];
    if(!Array.isArray(draft))throw new Error('Invalid saved attachments.');
    const groups=[draft,...queued.map(entry=>entry.blocks)],images=[];let bytes=0;
    for(const [index,blocks]of groups.entries()){
        let count=0,total=0;
        for(const block of blocks){
            if(!block||typeof block.token!=='string'||!block.token||!['image','text'].includes(block.kind)||block.kind==='text'&&typeof block.text!=='string')throw new Error('Invalid saved attachment.');
            if(block.kind!=='image')continue;
            if(!(block.blob instanceof Blob)||block.blob.size>8*MiB)throw new Error('Invalid or oversized saved image.');
            count++;total+=block.blob.size;bytes+=block.blob.size;images.push(block);
        }
        if(index===0&&(count>8||total>16*MiB))throw new Error('Saved draft images exceed the size limit.');
    }
    if(bytes>budget)throw new Error('Saved images exceed the restore memory budget.');
    const copies=[];
    // WebKit can invalidate IDB-backed Blob handles when their record is
    // replaced. Own bytes before autosave, serially to bound transient memory.
    for(const block of images)copies.push(new Blob([await block.blob.arrayBuffer()],{type:block.blob.type}));
    for(const [index,block]of images.entries())block.blob=copies[index];
    return {record,bytes};
}
function checked(record) {
    validateSession(record);
    if(new TextEncoder().encode(JSON.stringify(record)).length>LIMIT) throw new Error('Session exceeds 64 MiB; export files and start a new conversation.');
    return record;
}
export async function openStateStore() {
    const db=await new Promise((resolve,reject)=>{
        const request=indexedDB.open(DB,1);
        request.onupgradeneeded=()=>{
            const database=request.result;
            database.createObjectStore('workspaces',{keyPath:'id'});
            const sessions=database.createObjectStore('sessions',{keyPath:'id'});sessions.createIndex('workspaceId','workspaceId');
            database.createObjectStore('ui',{keyPath:'key'});
        };
        request.onsuccess=()=>{request.result.onversionchange=()=>request.result.close();resolve(request.result);};
        request.onerror=()=>reject(new Error('Cannot open browser state storage.'));
        request.onblocked=()=>reject(new Error('Close other app windows to upgrade storage.'));
    });
    async function write(name,fn) {
        return new Promise((resolve,reject)=>{
            const tx=db.transaction(name,'readwrite');let failure,result;
            tx.oncomplete=()=>resolve(result);
            tx.onabort=()=>reject(failure ?? tx.error ?? new Error('State transaction aborted.'));
            tx.onerror=()=>{};
            try{fn(tx.objectStore(name),value=>{result=value;},error=>{failure=error;tx.abort();});}catch(error){failure=error;tx.abort();}
        });
    }
    async function all(name) {
        return new Promise((resolve,reject)=>{const tx=db.transaction(name);const request=tx.objectStore(name).getAll();tx.oncomplete=()=>resolve(request.result);tx.onabort=()=>reject(tx.error);});
    }
    return {
        async load(){const [workspaces,sessions,ui]=await Promise.all(['workspaces','sessions','ui'].map(all));return {workspaces,sessions,ui:ui.find(r=>r.key==='main') ?? null};},
        async prepareSession(record,budget){
            try{return await prepareImages(record,budget);}
            catch(error){
                if(error.name!=='NotFoundError')throw error;
                // A concurrent replacement may retire this read's Blob handle.
                // Retry once, but never silently adopt a different revision.
                const fresh=await new Promise((resolve,reject)=>{
                    const tx=db.transaction('sessions'),request=tx.objectStore('sessions').get(record.id);
                    tx.oncomplete=()=>resolve(request.result);tx.onabort=()=>reject(tx.error);
                });
                if(!fresh||fresh.revision!==record.revision)throw error;
                return prepareImages(fresh,budget);
            }
        },
        commitSession(snapshot,expectedRevision){
            checked(snapshot);const copy=structuredClone(snapshot);
            return write('sessions',(store,result,fail)=>{
                const get=store.get(copy.id);get.onsuccess=()=>{
                    if((get.result?.revision ?? 0)!==expectedRevision){fail(new Error('Session changed in another window. Reload before continuing; your draft is not overwritten.'));return;}
                    copy.revision=expectedRevision+1;copy.updatedAt=Date.now();store.put(copy);result(copy.revision);
                };
            });
        },
        createWorkspace(record){return write('workspaces',store=>store.add(structuredClone(record)));},
        commitWorkspace(record){return write('workspaces',store=>store.put(structuredClone(record)));},
        commitUi(ui){return write('ui',store=>store.put({...structuredClone(ui),key:'main'}));},
        deleteSession(id){return write('sessions',store=>store.delete(id));},
        close(){db.close();},
    };
}
export async function withWorkspaceLock(id,fn) {
    if(!navigator.locks?.request) throw new Error('This browser does not support safe workspace locking.');
    return navigator.locks.request(`harness:workspace:${id}`,{ifAvailable:true},lock=>{
        if(!lock) throw new Error('This workspace is in use in another window.');return fn();
    });
}

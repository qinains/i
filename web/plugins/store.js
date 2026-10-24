import {validateWorkspaceId,isTemporaryWorkspace} from '../workspaces.js';
const encoder=new TextEncoder(),LIMIT=1024*1024;
export function pluginId(id){if(typeof id!=='string'||!/^[a-z][a-z0-9.-]{0,63}$/u.test(id))throw new Error('Invalid plugin ID.');return id;}
export function persistentWorkspace(id){validateWorkspaceId(id);if(isTemporaryWorkspace(id))throw new Error('Plugins are unavailable in temporary workspaces.');return id;}
function ownerKey(owner){return [persistentWorkspace(owner.workspaceId),pluginId(owner.pluginId)];}
function kvKey(owner,key){if(typeof key!=='string'||!key.length||encoder.encode(key).length>128)throw new Error('Invalid plugin storage key.');return [...ownerKey(owner),key];}
export async function openPluginStore(){
    const db=await new Promise((resolve,reject)=>{
        const request=indexedDB.open('harness_plugins',1);
        request.onupgradeneeded=()=>{
            const db=request.result;db.createObjectStore('packages',{keyPath:['id','version','digest']});db.createObjectStore('active',{keyPath:'id'});
            db.createObjectStore('grants',{keyPath:['workspaceId','pluginId']});db.createObjectStore('meta',{keyPath:'key'});
            const kv=db.createObjectStore('kv',{keyPath:['workspaceId','pluginId','key']});kv.createIndex('owner',['workspaceId','pluginId']);kv.createIndex('pluginId','pluginId');
        };
        request.onsuccess=()=>{request.result.onversionchange=()=>request.result.close();resolve(request.result);};
        request.onerror=()=>reject(new Error('Cannot open plugin storage.'));request.onblocked=()=>reject(new Error('Close other app windows to open plugin storage.'));
    });
    function transaction(names,mode,fn){return new Promise((resolve,reject)=>{
        const tx=db.transaction(names,mode);let value,failure;
        tx.oncomplete=()=>resolve(value);tx.onabort=()=>reject(failure??tx.error??new Error('Plugin storage transaction aborted.'));tx.onerror=()=>{};
        const fail=error=>{failure=error;tx.abort();};
        try{fn(tx,result=>{value=result;},fail);}catch(error){fail(error);}
    });}
    const ownerRead=(owner,key)=>transaction(['kv'],'readonly',(tx,done)=>{tx.objectStore('kv').get(kvKey(owner,key)).onsuccess=event=>done(event.target.result?.value??null);});
    return {
        close:()=>db.close(),
        defaultInitialized:owner=>transaction(['meta'],'readonly',(tx,done)=>{tx.objectStore('meta').get('default:'+ownerKey(owner).join(':')).onsuccess=event=>done(event.target.result?.value===true);}),
        markDefaultInitialized:owner=>transaction(['meta'],'readwrite',(tx,done)=>{tx.objectStore('meta').put({key:'default:'+ownerKey(owner).join(':'),value:true});done();}),
        loadRegistry:()=>transaction(['packages','active','grants','meta'],'readonly',(tx,done)=>{
            const result={revision:0,packages:[],active:[],grants:[]};for(const name of ['packages','active','grants'])tx.objectStore(name).getAll().onsuccess=event=>{result[name]=event.target.result;};
            tx.objectStore('meta').get('revision').onsuccess=event=>{result.revision=event.target.result?.value??0;};done(result);
        }),
        commitRegistry:(change,expectedRevision)=>transaction(['packages','active','grants','meta'],'readwrite',(tx,done,fail)=>{
            tx.objectStore('meta').get('revision').onsuccess=event=>{
                try{
                    const revision=event.target.result?.value??0;if(!Number.isSafeInteger(revision)||revision!==expectedRevision)throw new Error('Plugin registry changed in another window. Reload the plugin list.');
                    for(const [name,put,del]of [['packages','putPackages','deletePackages'],['active','putActive','deleteActive'],['grants','putGrants','deleteGrants']]){
                        for(const record of change[put]??[])tx.objectStore(name).put(structuredClone(record));
                        for(const key of change[del]??[])tx.objectStore(name).delete(key);
                    }
                    tx.objectStore('meta').put({key:'revision',value:revision+1});done(revision+1);
                }catch(error){fail(error);}
            };
        }),
        kvGet:ownerRead,
        kvPut:(owner,key,value)=>{
            const entry={...owner,key,value:structuredClone(value)},encoded=JSON.stringify(value);
            if(encoded===undefined)throw new Error('Plugin storage requires JSON values.');
            const charge=encoder.encode(key).length+encoder.encode(encoded).length;
            kvKey(owner,key);if(charge>LIMIT)throw new Error('Plugin storage exceeds 1 MiB.');
            return transaction(['kv'],'readwrite',(tx,done,fail)=>{
                let total=charge;const store=tx.objectStore('kv'),request=store.index('owner').openCursor(IDBKeyRange.only(ownerKey(owner)));
                request.onsuccess=event=>{try{const cursor=event.target.result;if(cursor){if(cursor.value.key!==key)total+=encoder.encode(cursor.value.key).length+encoder.encode(JSON.stringify(cursor.value.value)).length;if(total>LIMIT)throw new Error('Plugin storage exceeds 1 MiB.');cursor.continue();}else{store.put(entry);done();}}catch(error){fail(error);}};
            });
        },
        kvDelete:(owner,key)=>transaction(['kv'],'readwrite',(tx,done)=>{tx.objectStore('kv').delete(kvKey(owner,key));done();}),
        kvClear:owner=>transaction(['kv'],'readwrite',(tx,done)=>{const request=tx.objectStore('kv').index('owner').openCursor(IDBKeyRange.only(ownerKey(owner)));request.onsuccess=event=>{const cursor=event.target.result;if(cursor){cursor.delete();cursor.continue();}else done();};}),
        clearPluginData:id=>transaction(['kv'],'readwrite',(tx,done)=>{const request=tx.objectStore('kv').index('pluginId').openCursor(IDBKeyRange.only(pluginId(id)));request.onsuccess=event=>{const cursor=event.target.result;if(cursor){cursor.delete();cursor.continue();}else done();};}),
    };
}

export const CATALOG_URL='https://qinains.github.io/i-plugins/index.json';
const PACKAGE_BASE='https://qinains.github.io/i-plugins/packages/';
const CATALOG_LIMIT=512*1024,PACKAGE_LIMIT=10*1024*1024,TIMEOUT=30000;
const encoder=new TextEncoder();

function object(value){
    if(!value||typeof value!=='object'||Array.isArray(value)||![Object.prototype,null].includes(Object.getPrototypeOf(value)))throw new Error('Invalid catalog object.');
    for(const key of Reflect.ownKeys(value)){
        if(typeof key!=='string'||['__proto__','prototype','constructor'].includes(key)||!Object.hasOwn(Object.getOwnPropertyDescriptor(value,key),'value'))throw new Error('Unsafe catalog property.');
    }
    return value;
}
function field(value,key){if(!Object.hasOwn(value,key))throw new Error(`Missing catalog field: ${key}.`);return value[key];}
function text(value,max){if(typeof value!=='string'||encoder.encode(value).length>max||!value.trim()||/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u.test(value))throw new Error('Invalid catalog metadata.');return value;}
function permissions(value){
    object(value);const workspace=field(value,'workspace'),storage=field(value,'storage');
    if(Object.keys(value).length!==2||!['none','read','read-write'].includes(workspace)||typeof storage!=='boolean')throw new Error('Invalid catalog permissions.');
    return Object.freeze({workspace,storage});
}
function plugin(value){
    object(value);
    const id=field(value,'id'),version=field(value,'version');
    if(typeof id!=='string'||!/^[a-z][a-z0-9.-]{0,63}$/u.test(id)||id.startsWith('builtin.'))throw new Error('Invalid catalog plugin ID.');
    if(typeof version!=='string'||version.length>32||!/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/u.test(version)||version.split('.').some(part=>Number(part)>4294967295))throw new Error('Invalid catalog plugin version.');
    const repository=text(field(value,'repository'),256),source_revision=field(value,'source_revision');
    if(!/^https:\/\/github\.com\/[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})\/[A-Za-z0-9_.-]{1,100}$/u.test(repository)||['.','..'].includes(repository.split('/').at(-1)))throw new Error('Invalid catalog repository.');
    if(typeof source_revision!=='string'||!/^[a-f0-9]{40}$/u.test(source_revision))throw new Error('Invalid catalog source revision.');
    const pkg=object(field(value,'package')),sha256=field(pkg,'sha256'),size=field(pkg,'size'),url=field(pkg,'url');
    // Compare the original string, not a URL-normalized path that can hide traversal.
    if(typeof sha256!=='string'||!/^[a-f0-9]{64}$/u.test(sha256)||url!==`${PACKAGE_BASE}${sha256}.hplugin.zip`)throw new Error('Invalid catalog package URL or digest.');
    if(!Number.isSafeInteger(size)||size<=0||size>PACKAGE_LIMIT)throw new Error('Invalid catalog package size.');
    return Object.freeze({id,name:text(field(value,'name'),256),version,description:text(field(value,'description'),4096),repository,source_revision,license:text(field(value,'license'),128),package:Object.freeze({url,sha256,size}),permissions:permissions(field(value,'permissions'))});
}
export function validateCatalog(value){
    object(value);if(field(value,'schema_version')!==1)throw new Error('Unsupported plugin catalog schema.');
    const entries=field(value,'plugins');if(!Array.isArray(entries)||entries.length>500)throw new Error('Invalid plugin catalog entries.');
    const ids=new Set(),plugins=Array.from(entries,item=>{const entry=plugin(item);if(ids.has(entry.id))throw new Error('Duplicate catalog plugin ID.');ids.add(entry.id);return entry;});
    const catalog={schema_version:1,plugins:Object.freeze(plugins)};
    if(encoder.encode(JSON.stringify(catalog)).length>CATALOG_LIMIT)throw new Error('Plugin catalog exceeds 512 KiB.');
    return Object.freeze(catalog);
}

async function operation(caller,fn){
    const controller=new AbortController(),abort=()=>controller.abort(caller.reason);
    if(caller){if(caller.aborted)abort();else caller.addEventListener('abort',abort,{once:true});}
    const timer=setTimeout(()=>controller.abort(new DOMException('Plugin catalog request timed out.','TimeoutError')),TIMEOUT);
    const signal=controller.signal;
    function wait(promise){
        if(signal.aborted){Promise.resolve(promise).catch(()=>{});return Promise.reject(signal.reason);}
        return new Promise((resolve,reject)=>{
            const onAbort=()=>reject(signal.reason);signal.addEventListener('abort',onAbort,{once:true});
            Promise.resolve(promise).then(resolve,reject).finally(()=>signal.removeEventListener('abort',onAbort));
        });
    }
    try{signal.throwIfAborted();return await fn(signal,wait);}finally{clearTimeout(timer);caller?.removeEventListener('abort',abort);}
}
async function download(url,{fetchImpl,signal,wait,limit,expected,json=false}){
    const response=await wait(fetchImpl(url,{signal,credentials:'omit',cache:'no-store',referrerPolicy:'no-referrer',redirect:'error',mode:'cors'}));
    let reader;
    try{
        if(!response.ok||response.redirected||(response.url&&response.url!==url))throw new Error('Plugin catalog download failed.');
        if(json&&!/^application\/json(?:\s*;\s*charset\s*=\s*(?:utf-8|"utf-8"))?\s*$/iu.test(response.headers.get('content-type')??''))throw new Error('Plugin catalog requires JSON content type.');
        // Fetch decodes bodies, but Content-Length is a wire size and CORS may
        // hide Content-Encoding. Only use the header as an early upper bound.
        const rawLength=response.headers.get('content-length');let declared;
        if(rawLength!==null){
            if(!/^\d+$/u.test(rawLength))throw new Error('Invalid download content length.');
            declared=Number(rawLength);if(!Number.isSafeInteger(declared)||declared>limit)throw new Error('Plugin download exceeds registered size or limit.');
        }
        if(!response.body?.getReader)throw new Error('Plugin download has no readable body.');
        reader=response.body.getReader();const buffer=new Uint8Array(expected??limit);let length=0;
        while(true){
            const {done,value}=await wait(reader.read());if(done)break;
            if(!(value instanceof Uint8Array))throw new Error('Invalid plugin download stream.');
            length+=value.length;if(length>limit||(expected!==undefined&&length>expected))throw new Error('Plugin download exceeds registered size or limit.');
            buffer.set(value,length-value.length);
        }
        if(expected!==undefined&&length!==expected)throw new Error('Plugin download size mismatch.');
        return buffer.slice(0,length);
    }catch(error){
        // Do not let a stalled cancellation hide the original error or timeout.
        try{Promise.resolve(reader?reader.cancel():response.body?.cancel()).catch(()=>{});}catch{}
        throw error;
    }finally{try{reader?.releaseLock();}catch{}}
}
export function fetchCatalog({fetchImpl=fetch,signal}={}){
    return operation(signal,async(combined,wait)=>{
        const bytes=await download(CATALOG_URL,{fetchImpl,signal:combined,wait,limit:CATALOG_LIMIT,json:true});
        return validateCatalog(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)));
    });
}
export async function previewCatalogPlugin(manager,entry,{fetchImpl=fetch,signal}={}){
    // Snapshot before the first await: a caller may mutate its original entry.
    const selected=plugin(entry);
    return operation(signal,async(combined,wait)=>{
        const bytes=await download(selected.package.url,{fetchImpl,signal:combined,wait,limit:PACKAGE_LIMIT,expected:selected.package.size});
        const hash=[...new Uint8Array(await wait(crypto.subtle.digest('SHA-256',bytes)))].map(value=>value.toString(16).padStart(2,'0')).join('');
        if(hash!==selected.package.sha256)throw new Error('Catalog plugin package digest mismatch.');
        const preview=await wait(manager.preview(bytes)),manifest=preview?.manifest;
        if(!manifest||manifest.id!==selected.id||manifest.name!==selected.name||manifest.version!==selected.version)throw new Error('Catalog plugin manifest identity mismatch.');
        const declared=permissions(manifest.permissions);
        if(declared.workspace!==selected.permissions.workspace||declared.storage!==selected.permissions.storage)throw new Error('Catalog plugin manifest permissions mismatch.');
        return preview;
    });
}

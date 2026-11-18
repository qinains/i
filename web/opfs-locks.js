// Shared by the UI bridge and every generated WASM bridge in this realm.
const state=globalThis[Symbol.for('harness.browser.fileLocks')]??={leases:new WeakMap(),queues:new Map()};
function checked(root,mode){
    if(!['shared','exclusive'].includes(mode))throw new Error('Invalid file transaction mode.');
    const lease=state.leases.get(root);if(!lease)return null;
    if(!lease.active)throw new Error('File transaction is closed.');
    if(mode==='exclusive'&&lease.mode!=='exclusive')throw new Error('File transaction is read-only.');
    return lease;
}
function wait(promise,signal){
    signal.throwIfAborted();
    return new Promise((resolve,reject)=>{
        const aborted=()=>reject(signal.reason);signal.addEventListener('abort',aborted,{once:true});
        promise.then(resolve,reject).finally(()=>signal.removeEventListener('abort',aborted));
    });
}
async function localGate(key,signal){
    const previous=state.queues.get(key)??Promise.resolve();let unlock;
    const held=new Promise(resolve=>unlock=resolve),tail=previous.then(()=>held);state.queues.set(key,tail);
    tail.then(()=>{if(state.queues.get(key)===tail)state.queues.delete(key);});
    try{await wait(previous,signal);return unlock;}catch(error){unlock();throw error;}
}
export async function beginFileTransaction(root,id,mode,signal){
    if(checked(root,mode))throw new Error('Reuse the existing file transaction instead of opening a nested lease.');
    const cancellation=signal?.signal??signal,timeout=AbortSignal.timeout(15000);signal=cancellation?AbortSignal.any([cancellation,timeout]):timeout;
    signal.throwIfAborted();
    if(id==null&&root?.kind==='directory'&&typeof root.isSameEntry==='function')throw new Error('Open a managed workspace before accessing files.');
    if(id!=null&&!id.startsWith('temp-')&&!globalThis.navigator?.locks?.request)throw new Error('OPFS file locking requires Web Locks support.');
    const token=Object.freeze({}),lease={root,mode,active:true,release:null,done:null,closing:null,pending:new Set(),cleanup:()=>{}};
    if(id!=null&&globalThis.navigator?.locks?.request){
        let granted,failed;const ready=new Promise((resolve,reject)=>{granted=resolve;failed=reject;});
        const held=new Promise(resolve=>lease.release=resolve);
        lease.done=navigator.locks.request(`harness:files:${id}`,{mode,signal},async()=>{state.leases.set(token,lease);granted(token);await held;});
        lease.done.catch(failed);await ready;
    }else{
        // Temporary roots need only realm-local exclusion; serialize reads here too.
        lease.release=await localGate(id??root,signal);lease.done=Promise.resolve();state.leases.set(token,lease);
    }
    if(cancellation){
        const aborted=()=>{releaseFileTransaction(token);};cancellation.addEventListener('abort',aborted,{once:true});
        lease.cleanup=()=>cancellation.removeEventListener('abort',aborted);
        if(cancellation.aborted){await releaseFileTransaction(token);throw cancellation.reason;}
    }
    return token;
}
export function releaseFileTransaction(root){
    const lease=state.leases.get(root);if(!lease)throw new Error('Unknown file transaction.');
    if(lease.active){
        lease.active=false;lease.cleanup();
        // Closing a scope must not let another writer overtake an unawaited write.
        lease.closing=Promise.allSettled([...lease.pending]).then(()=>lease.release()).then(()=>lease.done);
    }
    return lease.closing;
}
export async function withFileTransaction(root,id,mode,fn,signal){
    (signal?.signal??signal)?.throwIfAborted();
    if(checked(root,mode))return fn(root);
    const lease=await beginFileTransaction(root,id,mode,signal);
    try{return await fn(lease);}finally{await releaseFileTransaction(lease);}
}
export function withFileRoot(root,id,mode,fn){
    const lease=checked(root,mode);
    if(lease){
        const operation=Promise.resolve().then(()=>fn(lease.root));lease.pending.add(operation);
        operation.then(()=>lease.pending.delete(operation),()=>lease.pending.delete(operation));return operation;
    }
    return withFileTransaction(root,id,mode,token=>fn(state.leases.get(token).root));
}

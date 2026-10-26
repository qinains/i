const manifestFields=['format_version','id','name','version','runtime','entry','sha256','permissions','tools'];
const verifiedFields=['manifest','digest','license','wasm','descriptors'];
function record(value,allowed,required=allowed){
    if(!value||typeof value!=='object'||Array.isArray(value)||![Object.prototype,null].includes(Object.getPrototypeOf(value)))throw new Error('Invalid plugin policy record.');
    const keys=Reflect.ownKeys(value);
    if(keys.some(key=>!allowed.includes(key)||!Object.hasOwn(Object.getOwnPropertyDescriptor(value,key),'value'))||required.some(key=>!Object.hasOwn(value,key)))throw new Error('Unsupported plugin policy fields.');
    return value;
}

// This policy consumes the trusted Rust inspector's result, not plugin claims.
// Rust validates Wasm imports/start/memory/ABI and the actual module bytes.
export function assessPlugin(verified){
    record(verified,verifiedFields,['manifest','digest']);
    const manifest=record(verified.manifest,manifestFields);
    if(manifest.format_version!==1||manifest.runtime!=='harness-wasm-v1'||manifest.entry!=='plugin.wasm')throw new Error('Unsupported plugin format/runtime/entry.');
    if(typeof verified.digest!=='string'||!/^[a-f0-9]{64}$/u.test(verified.digest)||verified.digest!==manifest.sha256)throw new Error('Plugin policy module digest mismatch.');
    const permissions=record(manifest.permissions,['workspace','storage']);
    if(!['none','read','read-write'].includes(permissions.workspace)||typeof permissions.storage!=='boolean')throw new Error('Unsupported plugin permissions.');
    // Storage is host-mediated plugin KV, never direct localStorage/IndexedDB access.
    return Object.freeze({workspace:permissions.workspace,pluginStorage:permissions.storage,apiCredentials:false,connectionSettings:false,browserStorage:false,network:false,shell:false,imports:false});
}

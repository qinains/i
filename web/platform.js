import {readChatCompletion} from './sse.js';
import {readAnthropicMessage} from './anthropic-sse.js';
import {readResponses} from './responses-sse.js';
import {modelDeadline} from './model-deadline.js';
import {validateCustomHeaders} from './transport.js';
import {validateConnectionPlugin} from './connection-plugin.js';
import {validateWorkspaceId,isTemporaryWorkspace,browserPath} from './workspaces.js';
import {MemoryDirectory} from './memory-fs.js';
import {beginFileTransaction,releaseFileTransaction,withFileTransaction,withFileRoot} from './opfs-locks.js';
const FILE_LIMIT = 8 * 1024 * 1024;
const HTTP_LIMIT = 48 * 1024 * 1024;

export function validatePath(path) {
    if (typeof path !== 'string' || path.startsWith('/') || /[\\\0:]/u.test(path) || path.split('/').includes('..')) {
        throw new Error('Path must be workspace-relative and cannot contain ..');
    }
    return path.split('/').filter(part => part && part !== '.');
}
export async function opfsOpen() {
    return opfsOpenWorkspace('default');
}
export async function opfsOpenWorkspace(id) {
    validateWorkspaceId(id);
    if(storage.roots.has(id))return storage.roots.get(id);
    const pending=openRoot(id).then(root=>{storage.ids.set(root,id);return root;});storage.roots.set(id,pending);
    try{return await pending;}catch(error){if(storage.roots.get(id)===pending)storage.roots.delete(id);throw error;}
}
const storage=globalThis[Symbol.for('harness.browser.files') ]??= {roots:new Map(),ids:new WeakMap(),tmp:new Map()};
async function openRoot(id){
    if(isTemporaryWorkspace(id))return new MemoryDirectory();
    if (!globalThis.isSecureContext || !navigator.storage?.getDirectory) {
        throw new Error('OPFS requires HTTPS or localhost and a supported browser.');
    }
    const root = await navigator.storage.getDirectory();
    if(id==='default')return root.getDirectoryHandle('workspace', {create: true});
    const managed=await root.getDirectoryHandle('harness',{create:true});
    return (await managed.getDirectoryHandle('workspaces',{create:true})).getDirectoryHandle(id,{create:true});
}
export async function opfsClearTmp(id,signal){validateWorkspaceId(id);return opfsWithTransaction(await opfsOpenWorkspace(id),'exclusive',()=>storage.tmp.get(id)?.clear(),signal);}
function mounted(root,value){
    const {mount,path}=browserPath(value);
    if(mount==='tmp'){
        const id=storage.ids.get(root);if(!id)throw new Error('Temporary mount is unavailable.');
        if(!storage.tmp.has(id))storage.tmp.set(id,new MemoryDirectory(isTemporaryWorkspace(id)?root.budget:undefined));
        return [storage.tmp.get(id),path];
    }
    return [root,path];
}
async function parent(root, path, create = false) {
    [root,path]=mounted(root,path);
    const parts = validatePath(path);
    const name = parts.pop();
    if (!name) throw new Error('Expected a file path');
    let dir = root;
    for (const part of parts) dir = await dir.getDirectoryHandle(part, {create});
    return [dir, name];
}
async function readRaw(root, path, limit) {
    const [dir, name] = await parent(root, path);
    const file = await (await dir.getFileHandle(name)).getFile();
    if (file.size > limit) throw new Error('File exceeds byte limit');
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (bytes.length > limit) throw new Error('File exceeds byte limit');
    return bytes;
}
async function writeRaw(root, path, content) {
    const [dir, name] = await parent(root, path, true);
    let file,created=false,writer;
    try{file=await dir.getFileHandle(name);}catch(error){
        if(error.name!=='NotFoundError')throw error;
        file=await dir.getFileHandle(name,{create:true});created=true;
    }
    try {
        writer=await file.createWritable();
        await writer.write(content);
        await writer.close();
    } catch (error) {
        try { await writer?.abort(); } catch { /* Preserve the original write failure. */ }
        // Existing committed files must survive failure. Only remove a new,
        // uncommitted placeholder while the exclusive lease is still held.
        if(created)try{await dir.removeEntry(name);}catch{ /* Preserve the original write failure. */ }
        throw error;
    }
}
async function mkdirRaw(root, path) {
    [root,path]=mounted(root,path);
    let dir = root;
    for (const part of validatePath(path)) dir = await dir.getDirectoryHandle(part, {create: true});
}
async function removeRaw(root, path) {
    const [dir, name] = await parent(root, path);
    await dir.removeEntry(name);
}
async function listRaw(root, path) {
    const original=browserPath(path);[root,path]=mounted(root,path);
    let dir = root;
    for (const part of validatePath(path)) dir = await dir.getDirectoryHandle(part);
    const prefix = (original.mount==='tmp'?'/tmp/':'')+validatePath(path).join('/');
    const paths = [];
    for await (const name of dir.keys()) paths.push(prefix ? `${prefix.replace(/\/$/u,'')}/${name}` : name);
    return paths.sort();
}
async function metadataRaw(root, path) {
    if (!browserPath(path).path.length) return {len: 0, is_dir: true, is_file: false};
    try {
        const [dir, name] = await parent(root, path);
        try {
            const file = await (await dir.getFileHandle(name)).getFile();
            return {len: file.size, is_file: true, is_dir: false};
        } catch (error) {
            if (error.name !== 'TypeMismatchError') throw error;
            await dir.getDirectoryHandle(name);
            return {len: 0, is_file: false, is_dir: true};
        }
    } catch (error) {
        if (error.name === 'NotFoundError') return null;
        throw error;
    }
}
export function opfsBegin(root,mode,signal){return beginFileTransaction(root,storage.ids.get(root),mode,signal);}
export function opfsRelease(root){return releaseFileTransaction(root);}
export function opfsWithTransaction(root,mode,fn,signal){return withFileTransaction(root,storage.ids.get(root),mode,fn,signal);}
function files(root,mode,fn){return withFileRoot(root,storage.ids.get(root),mode,fn);}
export async function opfsRead(root,path,limit){return files(root,'shared',raw=>readRaw(raw,path,limit));}
export async function opfsFileVersion(root,path,signal){
    return opfsWithTransaction(root,'shared',lease=>files(lease,'shared',async raw=>{
        let bytes;try{bytes=await readRaw(raw,path,FILE_LIMIT);}catch(error){if(error.name==='NotFoundError')return null;throw error;}
        const digest=new Uint8Array(await crypto.subtle.digest('SHA-256',bytes));
        return Array.from(digest,value=>value.toString(16).padStart(2,'0')).join('');
    }),signal);
}
export async function opfsWriteIfVersion(root,path,bytes,expected){
    if(bytes.length>FILE_LIMIT)throw new Error('File exceeds 8 MiB');
    if(expected!==undefined&&expected!==null&&!(typeof expected==='string'&&/^[a-f0-9]{64}$/u.test(expected)))throw new Error('Invalid file version.');
    const content=bytes.slice();
    return opfsWithTransaction(root,'exclusive',async lease=>{
        if(expected!==undefined&&await opfsFileVersion(lease,path)!==expected)throw new Error('Upload conflict: target file changed. Nothing was overwritten; select the file again.');
        return files(lease,'exclusive',raw=>writeRaw(raw,path,content));
    });
}
export async function opfsWrite(root,path,bytes){
    if(bytes.length>FILE_LIMIT)throw new Error('File exceeds 8 MiB');
    // Copy borrowed WASM memory before waiting for any lock or handle.
    const content=bytes.slice();return files(root,'exclusive',raw=>writeRaw(raw,path,content));
}
export async function opfsMkdir(root,path){return files(root,'exclusive',raw=>mkdirRaw(raw,path));}
export async function opfsRemove(root,path){return files(root,'exclusive',raw=>removeRaw(raw,path));}
export async function opfsList(root,path){return files(root,'shared',raw=>listRaw(raw,path));}
export async function opfsMetadata(root,path){return files(root,'shared',raw=>metadataRaw(raw,path));}
export function createController() { return new AbortController(); }
export function abortFetch(controller) { controller.abort(); }
export async function fetchModel(url, body, key, controller, options = {}, onEvent = () => {}) {
    if (new TextEncoder().encode(body).length > HTTP_LIMIT) throw new Error('Request exceeds 48 MiB');
    const request = JSON.parse(body),stream=request.stream===true;
    const requestId = globalThis.crypto.randomUUID();
    const headers = {'content-type': 'application/json'};
    const protocol=options.protocol ?? 'openai';
    if(!['openai','anthropic','responses'].includes(protocol)) throw new Error('Unsupported model protocol.');
    if(options.transport==='plugin'){
        validateConnectionPlugin({base_url:url,api_key:key,protocol,stream,custom_headers:options.customHeaders??''});
        headers['X-I-Local-Token']=key;
        delete request.apiKey;delete request.api_key;
        body=JSON.stringify(request);
    }else if(protocol==='anthropic') {
        if(key) headers['x-api-key']=key;
        headers['anthropic-version']='2023-06-01';
        if(options.transport!=='relay') headers['anthropic-dangerous-direct-browser-access']='true';
    } else if (key) headers.authorization = `Bearer ${key}`;
    headers.accept = stream ? 'text/event-stream' : 'application/json';

    const extra=validateCustomHeaders(options.customHeaders ?? '');
    if(options.transport==='relay' && Object.keys(extra).length) throw new Error('Custom headers are supported only in Direct mode.');
    Object.assign(headers,extra);

    if (options.transport === 'relay') {
        const relay = new URL('api/chat/completions',new URL('./',globalThis.location.href));
        if (relay.origin !== globalThis.location.origin) throw new Error('Relay must be same-origin');
        headers['x-harness-upstream'] = url;
        headers['x-harness-relay-token'] = options.relayToken;
        url = relay.href;
    }
    const notify = (type,text = '',progress = {}) => onEvent({type,request_id:requestId,text,...progress});
    const deadline=modelDeadline(controller.signal??controller),signal=deadline.signal;
    let reader;
    try {
        if (stream) notify('stream_start');
        const response = await fetch(url, {method: 'POST', headers, body, signal, redirect: 'error', credentials: 'omit'});
        if (!response.body) throw new Error('Empty model response');
        const raw=response.body.getReader();deadline.touch();
        reader={read:async()=>{const result=await raw.read();if(result.value?.byteLength)deadline.touch();return result;},cancel:()=>raw.cancel(),releaseLock:()=>raw.releaseLock()};
        if (response.ok && stream) {
            if (!response.headers.get('content-type')?.toLowerCase().includes('text/event-stream')) throw new Error('Expected SSE; disable Stream to retry manually.');
            const parser=protocol==='anthropic'?readAnthropicMessage:protocol==='responses'?readResponses:readChatCompletion;
            const result = await parser(reader,{signal,allowedTools:protocol!=='openai'?(request.tools??[]).map(tool=>tool.name):[],onDelta:text => notify('assistant_delta',text),onReasoning:text=>notify('reasoning_delta',text),onProgress:progress=>notify('stream_progress','',progress)});
            signal.throwIfAborted();
            notify('stream_end');
            return JSON.stringify(result);
        }
        const text = await readText(reader,signal);
        if (!response.ok) {
            let detail = text;
            for (const secret of [key,options.relayToken]) if (secret) detail = detail.split(secret).join('[redacted]');
            throw new Error(`API ${response.status}: ${detail.slice(0,2000)}`);
        }
        return text;
    } catch (error) {
        if (stream) notify('stream_error');
        await reader?.cancel().catch(() => {});
        let detail = error?.name==='TimeoutError'?'No model response data received for 30 minutes. Previously committed files are retained; retry manually or check the endpoint.':String(error.message ?? error);
        for (const secret of [key,options.relayToken]) if (secret) detail = detail.split(secret).join('[redacted]');
        if (error instanceof TypeError) detail += ' (Network request failed; check connectivity, endpoint and CORS in developer tools.)';
        throw new Error(detail);
    } finally {deadline.dispose();reader?.releaseLock();}
}
async function readText(reader,signal) {
    const chunks = [];
    let size = 0;
    try {
        for (;;) {
            const {done, value} = await reader.read();
            signal.throwIfAborted();
            if (done) break;
            size += value.length;
            if (size > HTTP_LIMIT) throw new Error('Response exceeds 48 MiB');
            chunks.push(value);
        }
    } catch (error) { await reader.cancel().catch(() => {}); throw error; }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
    return new TextDecoder().decode(bytes);
}

import {readChatCompletion} from './sse.js';
const FILE_LIMIT = 8 * 1024 * 1024;
const HTTP_LIMIT = 48 * 1024 * 1024;

export function validatePath(path) {
    if (typeof path !== 'string' || path.startsWith('/') || /[\\\0:]/u.test(path) || path.split('/').includes('..')) {
        throw new Error('Path must be workspace-relative and cannot contain ..');
    }
    return path.split('/').filter(part => part && part !== '.');
}
export async function opfsOpen() {
    if (!globalThis.isSecureContext || !navigator.storage?.getDirectory) {
        throw new Error('OPFS requires HTTPS or localhost and a supported browser.');
    }
    const root = await navigator.storage.getDirectory();
    return root.getDirectoryHandle('workspace', {create: true});
}
async function parent(root, path, create = false) {
    const parts = validatePath(path);
    const name = parts.pop();
    if (!name) throw new Error('Expected a file path');
    let dir = root;
    for (const part of parts) dir = await dir.getDirectoryHandle(part, {create});
    return [dir, name];
}
export async function opfsRead(root, path, limit) {
    const [dir, name] = await parent(root, path);
    const file = await (await dir.getFileHandle(name)).getFile();
    if (file.size > limit) throw new Error('File exceeds byte limit');
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (bytes.length > limit) throw new Error('File exceeds byte limit');
    return bytes;
}
export async function opfsWrite(root, path, bytes) {
    if (bytes.length > FILE_LIMIT) throw new Error('File exceeds 8 MiB');
    // Imported slices borrow WASM memory, which may grow while awaiting handles.
    const content = bytes.slice();
    const [dir, name] = await parent(root, path, true);
    const writer = await (await dir.getFileHandle(name, {create: true})).createWritable();
    try {
        await writer.write(content);
        await writer.close();
    } catch (error) {
        try { await writer.abort(); } catch { /* Preserve the original write failure. */ }
        throw error;
    }
}
export async function opfsMkdir(root, path) {
    let dir = root;
    for (const part of validatePath(path)) dir = await dir.getDirectoryHandle(part, {create: true});
}
export async function opfsRemove(root, path) {
    const [dir, name] = await parent(root, path);
    await dir.removeEntry(name);
}
export async function opfsList(root, path) {
    let dir = root;
    for (const part of validatePath(path)) dir = await dir.getDirectoryHandle(part);
    const prefix = validatePath(path).join('/');
    const paths = [];
    for await (const name of dir.keys()) paths.push(prefix ? `${prefix}/${name}` : name);
    return paths.sort();
}
export async function opfsMetadata(root, path) {
    if (!validatePath(path).length) return {len: 0, is_dir: true, is_file: false};
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
export function createController() { return new AbortController(); }
export function abortFetch(controller) { controller.abort(); }
export async function fetchModel(url, body, key, controller, options = {}, onEvent = () => {}) {
    if (new TextEncoder().encode(body).length > HTTP_LIMIT) throw new Error('Request exceeds 48 MiB');
    const stream = JSON.parse(body).stream === true;
    const requestId = globalThis.crypto.randomUUID();
    const signal = AbortSignal.any([controller.signal ?? controller,AbortSignal.timeout(180000)]);
    const headers = {'content-type': 'application/json'};
    if (key) headers.authorization = `Bearer ${key}`;
    headers.accept = stream ? 'text/event-stream' : 'application/json';
    if (options.transport === 'relay') {
        const relay = new URL('api/chat/completions',new URL('./',globalThis.location.href));
        if (relay.origin !== globalThis.location.origin) throw new Error('Relay must be same-origin');
        headers['x-harness-upstream'] = url;
        headers['x-harness-relay-token'] = options.relayToken;
        url = relay.href;
    }
    const notify = (type,text = '') => onEvent({type,request_id:requestId,text});
    let reader;
    try {
        if (stream) notify('stream_start');
        const response = await fetch(url, {method: 'POST', headers, body, signal, redirect: 'error', credentials: 'omit'});
        if (!response.body) throw new Error('Empty model response');
        reader = response.body.getReader();
        if (response.ok && stream) {
            if (!response.headers.get('content-type')?.toLowerCase().includes('text/event-stream')) throw new Error('Expected SSE; disable Stream to retry manually.');
            const result = await readChatCompletion(reader,{signal,onDelta:text => notify('assistant_delta',text)});
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
        let detail = String(error.message ?? error);
        for (const secret of [key,options.relayToken]) if (secret) detail = detail.split(secret).join('[redacted]');
        if (error instanceof TypeError) detail += ' (Network request failed; check connectivity, endpoint and CORS in developer tools.)';
        throw new Error(detail);
    } finally { reader?.releaseLock(); }
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

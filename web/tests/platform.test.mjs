import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import {validatePath, opfsWrite, opfsMetadata, fetchModel} from '../platform.js';
test('model timeout identifies the request deadline instead of a generic signal error',async()=>{
    const fetch=globalThis.fetch;globalThis.fetch=async()=>{throw new DOMException('signal timed out','TimeoutError');};
    try{await assert.rejects(fetchModel('http://127.0.0.1:1','{}','',new AbortController()),/No model response data received for 30 minutes/);}
    finally{globalThis.fetch=fetch;}
});

test('streaming reaches UI before completion and never retries a JSON response', async () => {
    let release, hits = 0;
    const gate = new Promise(resolve => {release = resolve;});
    const server = http.createServer(async (req,res) => {
        hits++; for await (const chunk of req) {}
        if (req.url === '/json') {res.setHeader('content-type','application/json'); res.end('{}'); return;}
        res.setHeader('content-type','text/event-stream');
        res.write('data: {"choices":[{"index":0,"delta":{"content":"first"},"finish_reason":null}]}\n\n');
        await gate;
        res.end('data: {"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n');
    });
    await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
    const url = `http://127.0.0.1:${server.address().port}`;
    try {
        let first;
        const seen = new Promise(resolve => {first = resolve;});
        const pending = fetchModel(url,JSON.stringify({stream:true}),'',new AbortController(),{},event => {
            if (event.type === 'assistant_delta') first(event.text);
        });
        const text = await Promise.race([seen,new Promise(resolve => setTimeout(() => resolve('TIMEOUT'),300))]);
        release(); await pending;
        assert.equal(text,'first');
        await assert.rejects(fetchModel(url+'/json','{"stream":true}','',new AbortController(),{}),/SSE/i);
        assert.equal(hits,2);
    } finally {release(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve));}
});

test('path traversal and platform prefixes are rejected', () => {
    for (const path of ['../x', '/x', 'a/../b', 'C:/x', 'a\\b', 'x\0']) {
        assert.throws(() => validatePath(path));
    }
    assert.deepEqual(validatePath('中文/a.txt'), ['中文', 'a.txt']);
});
test('failed close aborts and never reports successful write', async () => {
    let aborted = false;
    const file = {createWritable: async () => ({write: async () => {}, close: async () => {throw new Error('QuotaExceeded');}, abort: async () => {aborted = true;}})};
    const root = {getFileHandle: async () => file};
    await assert.rejects(opfsWrite(root, 'x', new Uint8Array([1])), /QuotaExceeded/);
    assert.equal(aborted, true);
});
test('metadata only treats NotFoundError as absent', async () => {
    const root = {getFileHandle: async () => {throw new DOMException('denied', 'NotAllowedError');}};
    await assert.rejects(opfsMetadata(root, 'x'), /denied/);
});
test('Fetch respects an already aborted request', async () => {
    const controller = new AbortController(); controller.abort();
    await assert.rejects(fetchModel('http://127.0.0.1:1', '{}', '', controller.signal));
});
test('HTTP errors redact keys before clipping at the display boundary', async () => {
    const key = 'PRIVATE_SECRET_KEY';
    const server = http.createServer(async (req,res) => {for await (const c of req) {} res.writeHead(401); res.end('x'.repeat(2000-key.length+1)+key);});
    await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
    try {
        await assert.rejects(fetchModel(`http://127.0.0.1:${server.address().port}`,'{}',key,new AbortController()),error => {
            assert.ok(!error.message.includes(key.slice(0,-1))); assert.ok(error.message.includes('[redacted]')); return true;
        });
    } finally {server.closeAllConnections(); await new Promise(resolve => server.close(resolve));}
});
test('write copies borrowed WASM bytes before its first await', async () => {
    const bytes = new Uint8Array([1,2]); let written;
    const root = {getFileHandle: async () => ({createWritable: async () => ({write: async data => {written = [...data];}, close: async () => {}})})};
    const result = opfsWrite(root,'x',bytes);
    bytes.fill(9);
    await result;
    assert.deepEqual(written,[1,2]);
});
test('Fetch rejects actual streamed responses larger than 48 MiB', async () => {
    const server = http.createServer(async (req,res) => {
        for await (const chunk of req) { /* Consume request before sending response. */ }
        res.writeHead(200,{'content-type':'application/json'});
        const chunk = Buffer.alloc(1024 * 1024,32);
        for (let i=0; i<49; i++) res.write(chunk);
        res.end();
    });
    await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
    try {
        await assert.rejects(fetchModel(`http://127.0.0.1:${server.address().port}`,'{}','',new AbortController()), /48 MiB/);
    } finally {server.closeAllConnections(); await new Promise(resolve => server.close(resolve));}
});

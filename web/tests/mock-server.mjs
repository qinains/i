import http from 'node:http';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRelayServer} from '../../server/relay.mjs';
const root = fileURLToPath(new URL('../', import.meta.url));
const call = (name, args, id) => ({choices: [{finish_reason: 'tool_calls', message: {role: 'assistant', content: null,
    tool_calls: [{id, type: 'function', function: {name, arguments: JSON.stringify(args)}}]}}]});
const final = content => ({choices: [{finish_reason: 'stop', message: {role: 'assistant', content}}]});
let releaseStream;
let pwaRevision = '', missingAsset = false;
const relays = new Map();
http.createServer(async (req, res) => {
    if (req.url === '/api/chat/completions') {
        if (!/^127\.0\.0\.1:\d+$/u.test(req.headers.host ?? '')) {res.writeHead(403); res.end(); return;}
        const origin = `http://${req.headers.host}`;
        if (!relays.has(origin)) relays.set(origin,await createRelayServer({token:'fixture-relay-token-'.repeat(3),origin,endpoints:['https://api.example/v1/chat/completions'],
            lookup:async () => [{address:'8.8.8.8',family:4}],connect:(_url,options,callback) => http.request('http://127.0.0.1:4174',{...options,lookup:undefined},callback)}));
        relays.get(origin).emit('request',req,res); return;
    }
    if (/^http:\/\/127\.0\.0\.1:\d+$/u.test(req.headers.origin ?? '')) {
        res.setHeader('access-control-allow-origin',req.headers.origin);
        res.setHeader('access-control-allow-methods','POST, OPTIONS');
        res.setHeader('access-control-allow-headers','authorization, content-type');
    }
    if (req.method === 'OPTIONS') {res.writeHead(204); res.end(); return;}
    if (req.url?.startsWith('/test-pwa-state')) {
        const url = new URL(req.url,'http://localhost'); pwaRevision = url.searchParams.get('version') ?? ''; missingAsset = url.searchParams.get('missing') === '1'; res.end('ok'); return;
    }
    if (req.url === '/release-stream') {releaseStream?.(); res.end('released'); return;}
    if (req.url === '/mock/v1/chat/completions') {
        let raw = ''; for await (const chunk of req) raw += chunk;
        const body = JSON.parse(raw);
        const task = body.messages.findLast(m => m.role === 'user')?.content ?? '';
        const last = body.messages.at(-1);
        if (task === 'STREAM') {
            res.writeHead(200,{'content-type':'text/event-stream'});
            res.write('data: {"choices":[{"index":0,"delta":{"content":"First visible"},"finish_reason":null}]}\n\n');
            await new Promise(resolve => {releaseStream = resolve; res.once('close',resolve);}); releaseStream = null;
            res.end('data: {"choices":[{"index":0,"delta":{"content":" then done"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'); return;
        }
        let result;
        if (task === 'WAIT') { await new Promise(resolve => setTimeout(resolve, 3000)); result = call('write', {path:'late.txt',content:'late'}, 'late'); }
        else if (last.role === 'tool' && last.content.startsWith('ERROR')) result = final('Tool refused as expected.');
        else if (task === 'SHELL' && last.role === 'user') result = call('shell', {command:'touch escape'}, 'shell');
        else if (task === 'TRUNCATE') {
            result = call('write', {path:'bad.txt',content:'bad'}, 'bad'); result.choices[0].finish_reason = 'length';
        } else if (last.role === 'user') result = call('write', {path:'nested/结果.txt',content:'hello\n'}, 'write');
        else if (last.tool_call_id === 'write') result = call('edit', {path:'nested/结果.txt',old:'hello',new:'world'}, 'edit');
        else if (last.tool_call_id === 'edit') result = call('download', {}, 'download');
        else result = final('All done. ZIP ready.');
        if (body.stream) {
            res.writeHead(200,{'content-type':'text/event-stream'});
            const choice = result.choices[0];
            const delta = {...choice.message};
            if (delta.tool_calls) delta.tool_calls = delta.tool_calls.map((tool,index) => ({...tool,index}));
            res.write(`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:null}]})}\n\n`);
            res.end(`data: ${JSON.stringify({choices:[{index:0,delta:{},finish_reason:choice.finish_reason}]})}\n\ndata: [DONE]\n\n`);
        } else {res.writeHead(200, {'content-type':'application/json'}); res.end(JSON.stringify(result));}
        return;
    }
    try {
        let requested = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
        if (requested.startsWith('/sub/')) requested = requested.slice(4);
        if (missingAsset && requested === '/icons/icon-512.png') throw new Error('fixture missing asset');
        const file = path.resolve(root, `.${requested === '/' ? '/index.html' : requested}`);
        if (!file.startsWith(root)) throw new Error('invalid path');
        const types = {'.html':'text/html','.js':'text/javascript','.css':'text/css','.wasm':'application/wasm','.webmanifest':'application/manifest+json','.svg':'image/svg+xml','.png':'image/png','.json':'application/json'};
        let bytes = await readFile(file);
        if (requested === '/service-worker.js' && pwaRevision) bytes = Buffer.from(bytes.toString().replace(/"version":"[a-f0-9]+"/u,`"version":"fixture-${pwaRevision}"`));
        res.writeHead(200, {'content-type':types[path.extname(file)] ?? 'application/octet-stream'}); res.end(bytes);
    } catch { res.writeHead(404); res.end('Not found'); }
}).listen(4173, '127.0.0.1');
http.createServer(async (req,res) => {
    if (req.method === 'OPTIONS') {res.writeHead(401); res.end(); return;}
    let raw = ''; for await (const chunk of req) raw += chunk;
    if (req.headers.authorization === 'Bearer RETURN_401') {res.writeHead(401,{'content-type':'application/json'}); res.end('{"error":"unauthorized"}'); return;}
    if (raw && JSON.parse(raw).stream) {
        res.writeHead(200,{'content-type':'text/event-stream'});
        res.write('data: {"choices":[{"index":0,"delta":{"content":"Relay first"},"finish_reason":null}]}\n\n');
        await new Promise(resolve => {releaseStream = resolve; res.once('close',resolve);}); releaseStream = null;
        res.end('data: {"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n');
    } else {res.writeHead(200,{'content-type':'application/json'}); res.end(JSON.stringify(final('no CORS')));}
}).listen(4174,'127.0.0.1');

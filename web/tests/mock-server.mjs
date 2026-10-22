import http from 'node:http';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const call = (name, args, id) => ({choices: [{finish_reason: 'tool_calls', message: {role: 'assistant', content: null,
    tool_calls: [{id, type: 'function', function: {name, arguments: JSON.stringify(args)}}]}}]});
const final = content => ({choices: [{finish_reason: 'stop', message: {role: 'assistant', content}}]});
http.createServer(async (req, res) => {
    if (req.url === '/mock/v1/chat/completions') {
        let raw = ''; for await (const chunk of req) raw += chunk;
        const body = JSON.parse(raw);
        const task = body.messages.findLast(m => m.role === 'user')?.content ?? '';
        const last = body.messages.at(-1);
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
        res.writeHead(200, {'content-type':'application/json'}); res.end(JSON.stringify(result)); return;
    }
    try {
        const requested = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
        const file = path.resolve(root, `.${requested === '/' ? '/index.html' : requested}`);
        if (!file.startsWith(root)) throw new Error('invalid path');
        const types = {'.html':'text/html','.js':'text/javascript','.css':'text/css','.wasm':'application/wasm'};
        const bytes = await readFile(file);
        res.writeHead(200, {'content-type':types[path.extname(file)] ?? 'application/octet-stream'}); res.end(bytes);
    } catch { res.writeHead(404); res.end('Not found'); }
}).listen(4173, '127.0.0.1');
http.createServer((req,res) => {res.writeHead(200,{'content-type':'application/json'}); res.end(JSON.stringify(final('no CORS')));}).listen(4174,'127.0.0.1');

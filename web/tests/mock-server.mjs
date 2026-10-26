import http from 'node:http';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {forwardRustRelay} from './rust-relay-fixture.mjs';
import {sendResponses} from './responses-fixture.mjs';
const root = fileURLToPath(new URL('../', import.meta.url));
const call = (name, args, id) => ({choices: [{finish_reason: 'tool_calls', message: {role: 'assistant', content: null,
    tool_calls: [{id, type: 'function', function: {name, arguments: JSON.stringify(args)}}]}}]});
const final = content => ({choices: [{finish_reason: 'stop', message: {role: 'assistant', content}}]});
let releaseStream;
let pwaRevision = '', missingAsset = false;
function anthropicResponse(body) {
    const text=body.messages.filter(message=>message.role==='user').flatMap(message=>message.content).filter(block=>block.type==='text').map(block=>block.text).at(-1) ?? '';
    const results=body.messages.flatMap(message=>message.content).filter(block=>block.type==='tool_result');
    let content,stop_reason='end_turn';
    if(text==='ANTHROPIC_MAKE' && !results.length || text==='ANTHROPIC_TRUNCATE' || text==='ANTHROPIC_BAD_BATCH') {
        content=[{type:'thinking',thinking:'private reasoning',signature:'fixture-signature'}, {type:'tool_use',id:'anthropic_write',name:'write',input:{path:'anthropic.txt',content:'native messages\n'}}];stop_reason='tool_use';
        if(text==='ANTHROPIC_BAD_BATCH')content.push({type:'tool_use',id:'forbidden',name:'shell',input:{command:'touch forbidden'}});
    } else content=[{type:'text',text:results.length?'Anthropic done.':'Anthropic connected.'}];
    return {id:'msg_fixture',type:'message',role:'assistant',model:'fixture',content,stop_reason,stop_sequence:null,usage:{input_tokens:1,output_tokens:2}};
}
function sendAnthropic(res,body) {
    const result=anthropicResponse(body);
    if(!body.stream){res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify(result));return;}
    res.writeHead(200,{'content-type':'text/event-stream'});
    const send=(type,rest={})=>res.write(`event: ${type}\ndata: ${JSON.stringify({type,...rest})}\n\n`);
    send('message_start',{message:{...result,content:[],stop_reason:null,usage:{input_tokens:1,output_tokens:0}}});
    for(const [index,block] of result.content.entries()) {
        const start=block.type==='tool_use'?{...block,input:{}}:block.type==='thinking'?{...block,thinking:'',signature:''}:{...block,text:''};
        send('content_block_start',{index,content_block:start});
        if(block.type==='tool_use')send('content_block_delta',{index,delta:{type:'input_json_delta',partial_json:JSON.stringify(block.input)}});
        else if(block.type==='thinking') {
            send('content_block_delta',{index,delta:{type:'thinking_delta',thinking:block.thinking}});
            send('content_block_delta',{index,delta:{type:'signature_delta',signature:block.signature}});
        } else send('content_block_delta',{index,delta:{type:'text_delta',text:block.text}});
        send('content_block_stop',{index});
    }
    const truncate=JSON.stringify(body.messages).includes('ANTHROPIC_TRUNCATE');
    if(!truncate){send('message_delta',{delta:{stop_reason:result.stop_reason,stop_sequence:null},usage:{output_tokens:2}});send('message_stop');}
    res.end();
}
http.createServer(async (req, res) => {
    if (req.url === '/api/chat/completions') {
        if (!/^127\.0\.0\.1:\d+$/u.test(req.headers.host ?? '')) {res.writeHead(403); res.end(); return;}
        await forwardRustRelay(req,res); return;
    }
    if (/^http:\/\/127\.0\.0\.1:\d+$/u.test(req.headers.origin ?? '')) {
        res.setHeader('access-control-allow-origin',req.headers.origin);
        res.setHeader('access-control-allow-methods','POST, OPTIONS');
        res.setHeader('access-control-allow-headers','authorization, content-type, x-api-key, anthropic-version, anthropic-dangerous-direct-browser-access');
    }
    if (req.method === 'OPTIONS') {res.writeHead(204); res.end(); return;}
    if (req.url?.startsWith('/test-pwa-state')) {
        const url = new URL(req.url,'http://localhost'); pwaRevision = url.searchParams.get('version') ?? ''; missingAsset = url.searchParams.get('missing') === '1'; res.end('ok'); return;
    }
    if (req.url === '/release-stream') {releaseStream?.(); res.end('released'); return;}
    if(req.url==='/mock/v1/responses'){
        let raw='';for await(const chunk of req)raw+=chunk;const body=JSON.parse(raw);
        if(req.headers.authorization!=='Bearer TEST_RESPONSES_KEY'||!Array.isArray(body.input)||body.messages||body.max_tokens||!body.max_output_tokens||body.store!==false||body.tools?.some(tool=>tool.function||!tool.parameters)){res.writeHead(400);res.end('invalid native Responses request');return;}
        sendResponses(res,body);return;
    }
    if(req.url==='/mock/v1/messages') {
        let raw='';for await(const chunk of req)raw+=chunk;
        const body=JSON.parse(raw);
        if(req.headers.authorization || req.headers['x-api-key']!=='TEST_ANTHROPIC_KEY' || req.headers['anthropic-version']!=='2023-06-01' || req.headers['anthropic-dangerous-direct-browser-access']!=='true' || !Array.isArray(body.system) || body.tools && !body.tools.every(tool=>tool.input_schema && !tool.function)) {
            res.writeHead(400);res.end('invalid native Anthropic request');return;
        }
        sendAnthropic(res,body);return;
    }
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
        else if (last.tool_call_id === 'write') result = call('edit', {path:'nested/结果.txt',edits:[{old:'hello',new:'world'}]}, 'edit');
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
    if(req.url==='/v1/responses'){
        if(req.headers.authorization!=='Bearer TEST_RESPONSES_KEY'||req.headers['x-api-key']){res.writeHead(400);res.end('invalid Responses relay auth');return;}
        sendResponses(res,JSON.parse(raw));return;
    }
    if(req.url==='/v1/messages') {
        if(req.headers.authorization || req.headers['x-api-key']!=='TEST_ANTHROPIC_KEY' || req.headers['anthropic-version']!=='2023-06-01' || req.headers['anthropic-dangerous-direct-browser-access']) {res.writeHead(400);res.end('invalid native Anthropic relay request');return;}
        sendAnthropic(res,JSON.parse(raw));return;
    }
    if (req.headers.authorization === 'Bearer RETURN_401') {res.writeHead(401,{'content-type':'application/json'}); res.end('{"error":"unauthorized"}'); return;}
    if (raw && JSON.parse(raw).stream) {
        res.writeHead(200,{'content-type':'text/event-stream'});
        res.write('data: {"choices":[{"index":0,"delta":{"content":"Relay first"},"finish_reason":null}]}\n\n');
        await new Promise(resolve => {releaseStream = resolve; res.once('close',resolve);}); releaseStream = null;
        res.end('data: {"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n');
    } else {res.writeHead(200,{'content-type':'application/json'}); res.end(JSON.stringify(final('no CORS')));}
}).listen(4174,'127.0.0.1');

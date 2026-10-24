import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import {EventEmitter} from 'node:events';
import {fileURLToPath} from 'node:url';
import {createRelayServer} from '../relay.mjs';
const token = 'a'.repeat(48), origin = 'https://harness.example', endpoint = 'https://api.example/v1/chat/completions';
async function fixture(options={}) {
    const server = await createRelayServer({token,origin,endpoints:[endpoint],...options});
    await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
    return {server,url:`http://127.0.0.1:${server.address().port}/api/chat/completions`,close:async () => {server.closeAllConnections(); await new Promise(resolve => server.close(resolve));}};
}
const headers = {'content-type':'application/json',origin,'x-harness-relay-token':token,'x-harness-upstream':endpoint,authorization:'Bearer MODEL_KEY'};
test('Responses endpoints are explicit allowlisted and retain Bearer auth and native body',async()=>{
    const target='https://api.example/v1/responses';let observed,body;
    const upstream=http.createServer(async(req,res)=>{observed=req.headers;body='';for await(const chunk of req)body+=chunk;res.end('{"object":"response","status":"completed","output":[]}');});
    await new Promise(resolve=>upstream.listen(0,'127.0.0.1',resolve));
    let f;
    try{
        f=await fixture({endpoints:[target],lookup:async()=>[{address:'8.8.8.8',family:4}],connect:(_url,opts,cb)=>http.request(`http://127.0.0.1:${upstream.address().port}`,{...opts,lookup:undefined},cb)});
        const native={model:'fixture',input:'Hi',max_output_tokens:64,store:false,stream:false};
        const result=await fetch(f.url,{method:'POST',headers:{...headers,'x-harness-upstream':target},body:JSON.stringify(native)});
        assert.equal(result.status,200);assert.equal((await result.json()).object,'response');assert.deepEqual(JSON.parse(body),native);assert.equal(observed.authorization,'Bearer MODEL_KEY');assert.equal(observed['x-api-key'],undefined);
    }finally{await f?.close();upstream.closeAllConnections();await new Promise(resolve=>upstream.close(resolve));}
});
test('Anthropic allowlisted targets get only native credentials and fixed version',async()=>{
    const target='https://api.example/v1/messages';let observed;
    const upstream=http.createServer(async(req,res)=>{observed=req.headers;for await(const chunk of req){}res.end('{}');});
    await new Promise(resolve=>upstream.listen(0,'127.0.0.1',resolve));
    const f=await fixture({endpoints:[endpoint,target],lookup:async()=>[{address:'8.8.8.8',family:4}],connect:(_url,opts,cb)=>http.request(`http://127.0.0.1:${upstream.address().port}`,{...opts,lookup:undefined},cb)});
    try {
        const native={...headers,'x-api-key':'ANTHROPIC_KEY','anthropic-version':'evil','anthropic-dangerous-direct-browser-access':'true','x-arbitrary':'bad'};
        let result=await fetch(f.url,{method:'POST',headers:{...native,'x-harness-upstream':target},body:'{}'});assert.equal(result.status,200);await result.text();
        assert.equal(observed['x-api-key'],'ANTHROPIC_KEY');assert.equal(observed['anthropic-version'],'2023-06-01');assert.equal(observed.authorization,undefined);assert.equal(observed['anthropic-dangerous-direct-browser-access'],undefined);assert.equal(observed['x-arbitrary'],undefined);
        result=await fetch(f.url,{method:'POST',headers:native,body:'{}'});assert.equal(result.status,200);await result.text();
        assert.equal(observed.authorization,'Bearer MODEL_KEY');assert.equal(observed['x-api-key'],undefined);assert.equal(observed['anthropic-version'],undefined);
        result=await fetch(f.url,{method:'POST',headers:{...native,'x-harness-upstream':'https://api.example/v1/messages/evil'},body:'{}'});assert.equal(result.status,403);
    } finally {await f.close();upstream.closeAllConnections();await new Promise(resolve=>upstream.close(resolve));}
});
test('startup requires strong token, exact origin and HTTPS endpoint allowlist', async () => {
    for (const patch of [{token:''},{origin:'*'},{origin:'ftp://localhost'},{endpoints:['http://api.example/v1/chat/completions']},{endpoints:[endpoint+'?x=1']}]) await assert.rejects(createRelayServer({token,origin,endpoints:[endpoint],...patch}));
});
test('bounded requests, rate, concurrent requests, deadlines and disconnect abort', async () => {
    const f = await fixture({limits:{bytes:4,rate:1}});
    try {
        assert.equal((await fetch(f.url,{method:'POST',headers,body:'12345'})).status,413);
        assert.equal((await fetch(f.url,{method:'POST',headers,body:'{}'})).status,429);
    } finally {await f.close();}
    let upstreamAborted = false;
    const slow = await fixture({limits:{concurrency:1,deadline:100},lookup:async () => new Promise(() => {})});
    try {
        const pending = fetch(slow.url,{method:'POST',headers,body:'{}'});
        await new Promise(resolve => setTimeout(resolve,20));
        assert.equal((await fetch(slow.url,{method:'POST',headers,body:'{}'})).status,429);
        assert.equal((await pending).status,504);
    } finally {await slow.close();}
    const abort = await fixture({lookup:async () => [{address:'8.8.8.8',family:4}],connect:(url,opts) => {
        const req = new EventEmitter();
        opts.signal.addEventListener('abort',() => {upstreamAborted = true; req.emit('error',new Error('aborted'));});
        req.end = () => {}; return req;
    }});
    try {
        const controller = new AbortController();
        const request = fetch(abort.url,{method:'POST',headers,body:'{}',signal:controller.signal}).catch(() => {});
        await new Promise(resolve => setTimeout(resolve,30)); controller.abort(); await request;
        await new Promise(resolve => setTimeout(resolve,30)); assert.equal(upstreamAborted,true);
    } finally {await abort.close();}
});
test('auth, origin, endpoint, method, content type and SSRF fail before upstream', async () => {
    const f = await fixture({lookup:async () => [{address:'127.0.0.1',family:4}]});
    try {
        for (const [patch,status] of [[{'x-harness-relay-token':'bad'},401],[{origin:'https://evil.example'},403],[{'x-harness-upstream':endpoint+'/evil'},403],[{'content-type':'text/plain'},415],[{},502]]) {
            const r = await fetch(f.url,{method:'POST',headers:{...headers,...patch},body:'{}'}); assert.equal(r.status,status);
            assert.equal((await r.text()).includes('MODEL_KEY'),false);
        }
        assert.equal((await fetch(f.url)).status,405);
        assert.equal((await fetch(f.url.replace('/api/chat/completions','/.env'))).status,404);
    } finally {await f.close();}
});
test('static publish manifest serves assets, not repository or hidden files; response limit aborts', async () => {
    const f = await fixture({staticRoot:fileURLToPath(new URL('../../web/',import.meta.url))});
    const root = f.url.replace('/api/chat/completions','');
    try {
        assert.equal((await fetch(root+'/')).status,200);
        assert.match((await fetch(root+'/pkg/i_bg.wasm')).headers.get('content-type'),/application\/wasm/);
        for (const pathname of ['/.env','/tests/mock-server.mjs','/node_modules/@playwright/test/cli.js','/../server/main.mjs','/%2e%2e/server/main.mjs']) assert.equal((await fetch(root+pathname)).status,404);
    } finally {await f.close();}
    const upstream = http.createServer(async (req,res) => {for await (const c of req) {} res.end('response-too-large');});
    await new Promise(resolve => upstream.listen(0,'127.0.0.1',resolve));
    const limited = await fixture({limits:{bytes:4},lookup:async () => [{address:'8.8.8.8',family:4}],connect:(_url,opts,cb) => http.request(`http://127.0.0.1:${upstream.address().port}`,{...opts,lookup:undefined},cb)});
    try {
        await assert.rejects(async () => {const r = await fetch(limited.url,{method:'POST',headers,body:'{}'}); await r.text();});
    } finally {await limited.close(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve));}
});
test('upstream uses pinned DNS and restricted headers, preserves status; limits and redirect fail', async () => {
    let status = 200, observed;
    const upstream = http.createServer(async (req,res) => {observed = req.headers; for await (const c of req) {} res.writeHead(status,{'content-type':'text/event-stream'}); res.end('data: example\n\n');});
    await new Promise(resolve => upstream.listen(0,'127.0.0.1',resolve));
    const f = await fixture({lookup:async () => [{address:'8.8.8.8',family:4}],connect:(url,opts,cb) => {
        assert.equal(url.hostname,'api.example'); opts.lookup('api.example',{},(err,ip) => assert.equal(ip,'8.8.8.8'));
        return http.request(`http://127.0.0.1:${upstream.address().port}`,{...opts,lookup:undefined},cb);
    }});
    try {
        let r = await fetch(f.url,{method:'POST',headers:{...headers,cookie:'PRIVATE', 'x-arbitrary':'bad'},body:'{}'});
        assert.equal(r.status,200); assert.equal(await r.text(),'data: example\n\n');
        assert.equal(observed.authorization,'Bearer MODEL_KEY'); assert.equal(observed.cookie,undefined); assert.equal(observed.origin,undefined); assert.equal(observed['x-harness-relay-token'],undefined); assert.equal(observed['x-arbitrary'],undefined);
        status = 401; r = await fetch(f.url,{method:'POST',headers,body:'{}'}); assert.equal(r.status,401); await r.text();
        status = 302; r = await fetch(f.url,{method:'POST',headers,body:'{}'}); assert.equal(r.status,502);
    } finally {await f.close(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve));}
});

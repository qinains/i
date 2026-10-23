import http from 'node:http';
import https from 'node:https';
import {timingSafeEqual} from 'node:crypto';
import {readFile,realpath} from 'node:fs/promises';
import path from 'node:path';
import {once} from 'node:events';
import {resolvePublic} from './addresses.mjs';

const LIMIT = 48 * 1024 * 1024;
function endpoint(value) {
    const url = new URL(value);
    if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash || !(url.pathname.endsWith('/chat/completions') || url.pathname.endsWith('/v1/messages')) || (url.port && url.port !== '443')) throw new Error('Invalid HTTPS model endpoint');
    return url.href;
}
function authorized(actual,expected) {
    const value = Buffer.from(actual ?? '');
    return value.length === expected.length && timingSafeEqual(value,expected);
}
export async function createRelayServer({token,origin,endpoints,staticRoot,basePath='/',lookup,connect = https.request,limits = {}}) {
    const budget = {bytes:LIMIT,deadline:180000,concurrency:4,rate:60,...limits};
    for (const [key,max] of Object.entries({bytes:LIMIT,deadline:180000,concurrency:4,rate:60})) if (!Number.isSafeInteger(budget[key]) || budget[key] < 1 || budget[key] > max) throw new Error('Invalid relay limit');
    if (typeof token !== 'string' || Buffer.byteLength(token) < 32) throw new Error('A relay token of at least 32 bytes is required');
    const publicUrl = new URL(origin);
    if (publicUrl.origin !== origin || (publicUrl.protocol !== 'https:' && !(publicUrl.protocol === 'http:' && ['127.0.0.1','localhost','[::1]'].includes(publicUrl.hostname)))) throw new Error('Exact HTTPS public origin required');
    if (!Array.isArray(endpoints) || !endpoints.length) throw new Error('Endpoint allowlist required');
    const allowed = new Set(endpoints.map(endpoint));
    if (!/^\/(?:[A-Za-z0-9_-]+\/)*$/u.test(basePath)) throw new Error('Invalid base path');
    const expected = Buffer.from(token);
    let assets = new Set(), root;
    if (staticRoot) {
        root = await realpath(staticRoot);
        const manifest = JSON.parse(await readFile(path.join(root,'assets.json'),'utf8'));
        assets = new Set([...manifest.assets,'assets.json']);
        for (const asset of assets) if (typeof asset !== 'string' || asset.startsWith('/') || asset.split('/').some(p => p === '..' || p.startsWith('.')) || asset.includes('\\')) throw new Error('Unsafe publish manifest');
    }
    let active = 0, windowStart = Date.now(), count = 0;
    return http.createServer({maxHeaderSize:16384,requestTimeout:180000},async (req,res) => {
        res.setHeader('cache-control','no-store');
        const reply = (status,text) => {if (!res.headersSent) {res.writeHead(status,{'content-type':'text/plain; charset=utf-8'}); res.end(text);} else res.destroy();};
        if (req.url !== `${basePath}api/chat/completions`) {
            if (!root || !['GET','HEAD'].includes(req.method)) return reply(404,'Not found');
            try {
                const pathname = decodeURIComponent(new URL(req.url,'http://localhost').pathname);
                if (!pathname.startsWith(basePath)) return reply(404,'Not found');
                const asset = pathname.slice(basePath.length) || 'index.html';
                if (!assets.has(asset)) return reply(404,'Not found');
                const file = await realpath(path.join(root,asset));
                if (!file.startsWith(root+path.sep)) return reply(404,'Not found');
                const bytes = await readFile(file);
                const type = {'.html':'text/html; charset=utf-8','.js':'text/javascript','.css':'text/css','.wasm':'application/wasm','.json':'application/json','.webmanifest':'application/manifest+json','.svg':'image/svg+xml','.png':'image/png'}[path.extname(asset)] ?? 'application/octet-stream';
                res.writeHead(200,{'content-type':type,'x-content-type-options':'nosniff'}); return res.end(req.method === 'HEAD' ? undefined : bytes);
            } catch {return reply(404,'Not found');}
        }
        if (req.method !== 'POST') return reply(405,'POST required');
        if (req.headers.origin !== origin) return reply(403,'Origin denied');
        if (!authorized(req.headers['x-harness-relay-token'],expected)) return reply(401,'Relay authentication failed');
        if (!/^application\/json(?:\s*;|$)/iu.test(req.headers['content-type'] ?? '')) return reply(415,'JSON required');
        let target;
        try {target = endpoint(req.headers['x-harness-upstream']); if (!allowed.has(target)) throw new Error();}
        catch {return reply(403,'Upstream denied');}
        if (Date.now()-windowStart >= 60000) {windowStart = Date.now(); count = 0;}
        if (active >= budget.concurrency || count >= budget.rate) return reply(429,'Relay limit reached');
        count++; active++;
        const controller = new AbortController();
        let timedOut = false;
        const timeout = setTimeout(() => {timedOut = true; controller.abort(); if (!req.complete) req.destroy();},budget.deadline);
        const disconnected = () => {if (!res.writableFinished) controller.abort();};
        res.on('close',disconnected); req.on('aborted',disconnected);
        try {
            const chunks = []; let size = 0;
            for await (const chunk of req) {
                size += chunk.length;
                if (size > budget.bytes) {reply(413,'Request exceeds limit'); return;}
                chunks.push(chunk);
            }
            const body = Buffer.concat(chunks); JSON.parse(body.toString('utf8'));
            const url = new URL(target); controller.signal.throwIfAborted();
            const record = await Promise.race([resolvePublic(url.hostname,lookup),new Promise((_,reject) => controller.signal.addEventListener('abort',() => reject(new Error('Deadline exceeded')),{once:true}))]);
            controller.signal.throwIfAborted();
            const headers = {'content-type':'application/json',accept:'text/event-stream, application/json','accept-encoding':'identity','content-length':body.length};
            if(url.pathname.endsWith('/v1/messages')) {
                if(req.headers['x-api-key']) headers['x-api-key']=req.headers['x-api-key'];
                headers['anthropic-version']='2023-06-01';
            } else if (req.headers.authorization) headers.authorization = req.headers.authorization;
            await new Promise((resolve,reject) => {
                const upstream = connect(url,{method:'POST',headers,signal:controller.signal,agent:false,
                    lookup:(_host,opts,cb) => opts?.all ? cb(null,[record]) : cb(null,record.address,record.family)},async response => {
                    try {
                        if (response.statusCode >= 300 && response.statusCode < 400) throw new Error('Redirect denied');
                        res.writeHead(response.statusCode,{'content-type':response.headers['content-type'] ?? 'application/octet-stream','cache-control':'no-store','x-accel-buffering':'no'});
                        let received = 0;
                        for await (const chunk of response) {
                            received += chunk.length;
                            if (received > budget.bytes) throw new Error('Response exceeds limit');
                            if (!res.write(chunk)) await once(res,'drain',{signal:controller.signal});
                        }
                        res.end(); resolve();
                    } catch (error) {controller.abort(); response.destroy(); reject(error);}
                });
                upstream.on('error',reject); upstream.end(body);
            });
        } catch {reply(timedOut ? 504 : 502,'Relay request failed');}
        finally {clearTimeout(timeout); controller.abort(); res.off('close',disconnected); req.off('aborted',disconnected); active--;}
    });
}

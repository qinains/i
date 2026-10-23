import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {mkdtemp,cp,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium} from '../../web/node_modules/playwright/index.mjs';

const temporary=await mkdtemp(join(tmpdir(),'harness-cors-test-'));
const listen=async server=>{await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));return `http://127.0.0.1:${server.address().port}`;};
const app=createServer((req,res)=>{res.setHeader('content-type','text/html');res.end('<!doctype html><title>CORS test</title>');});
let release,authorization;
const upstream=createServer((req,res)=>{
    if(req.method==='OPTIONS'){res.writeHead(req.url==='/reject'?403:204);res.end();return;}
    authorization=req.headers.authorization;
    if(req.url==='/stream'){res.writeHead(200,{'content-type':'text/event-stream'});res.write('data: first\n\n');release=()=>res.end('data: last\n\n');return;}
    res.writeHead(req.url==='/unauthorized'?401:200,{'content-type':'application/json'});res.end(JSON.stringify({ok:req.url!=='/unauthorized'}));
});
let context;
try{
    const origin=await listen(app),api=await listen(upstream),source=join(temporary,'extension');
    await cp(new URL('../../dist/cors-helper/chromium/',import.meta.url),source,{recursive:true});
    const manifest=JSON.parse(await readFile(join(source,'manifest.json'),'utf8'));
    // Only the isolated fixture gets pregranted hosts; shipped ZIP stays optional.
    manifest.host_permissions=['http://127.0.0.1/*'];await writeFile(join(source,'manifest.json'),JSON.stringify(manifest));
    context=await chromium.launchPersistentContext(join(temporary,'profile'),{channel:'chromium',headless:true,args:[`--disable-extensions-except=${source}`,`--load-extension=${source}`]});
    const worker=context.serviceWorkers()[0]??await context.waitForEvent('serviceworker');
    const page=await context.newPage();await page.goto(origin);
    const other=await context.newPage();await other.goto(origin);
    const extension=worker.url().split('/')[2],control=await context.newPage();await control.goto(`chrome-extension://${extension}/popup.html`);
    const tabId=await worker.evaluate(async origin=>(await chrome.tabs.query({url:origin+'/*'}))[0].id,origin);
    const tabs=await worker.evaluate(async origin=>chrome.tabs.query({url:origin+'/*'}),origin);
    const pageIds=await page.evaluate(()=>location.href);assert.equal(pageIds,origin+'/');
    // The first created matching tab is the page under test.
    const targetId=Math.min(...tabs.map(tab=>tab.id));
    const fetchApi=(page,url)=>page.evaluate(async url=>{try{const res=await fetch(url,{method:'POST',credentials:'omit',headers:{'content-type':'application/json',authorization:'FAKE_TEST_KEY'},body:'{}'});return {status:res.status,text:await res.text()};}catch{return {blocked:true};}},url);
    assert.equal((await fetchApi(page,api+'/json')).blocked,true);
    const enable=endpoint=>control.evaluate(async({targetId,endpoint})=>{const {enableForTab}=await import('./actions.js');await enableForTab(targetId,endpoint);},{targetId,endpoint});
    await enable(api+'/json');assert.equal((await fetchApi(page,api+'/json')).status,200);assert.equal(authorization,'FAKE_TEST_KEY');
    assert.equal((await fetchApi(other,api+'/json')).blocked,true);
    assert.equal((await fetchApi(page,api+'/other')).blocked,true);
    await enable(api+'/unauthorized');assert.equal((await fetchApi(page,api+'/unauthorized')).status,401);
    await enable(api+'/reject');assert.equal((await fetchApi(page,api+'/reject')).blocked,true);
    await enable(api+'/stream');
    await page.evaluate(url=>{window.first='';window.complete=false;window.streamTask=(async()=>{const res=await fetch(url,{method:'POST',credentials:'omit',headers:{authorization:'FAKE_TEST_KEY'}}),reader=res.body.getReader();let part;while(!(part=await reader.read()).done)window.first+=new TextDecoder().decode(part.value);window.complete=true;})();},api+'/stream');
    await page.waitForFunction(()=>window.first.includes('first'));assert.equal(await page.evaluate(()=>window.complete),false);release();await page.evaluate(()=>window.streamTask);
    await control.evaluate(async targetId=>{const {disableForTab}=await import('./actions.js');await disableForTab(targetId);},targetId);
    assert.equal((await fetchApi(page,api+'/stream')).blocked,true);
    assert.ok(tabId>=0);console.log('PASS: real Chromium preflight, authorization, exact URL/tab isolation, 401, rejected OPTIONS, incremental SSE and disable.');
}finally{
    await context?.close();app.closeAllConnections();upstream.closeAllConnections();await Promise.all([new Promise(resolve=>app.close(resolve)),new Promise(resolve=>upstream.close(resolve))]);
    await rm(temporary,{recursive:true,force:true});
}

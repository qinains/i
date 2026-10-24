import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {mkdtemp,cp,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium} from '../../web/node_modules/playwright/index.mjs';

const temporary=await mkdtemp(join(tmpdir(),'harness-cors-test-'));
const listen=async server=>{await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));return `http://127.0.0.1:${server.address().port}`;};
let publicEndpoint='';
const app=createServer((req,res)=>{res.setHeader('content-type','text/html');res.end(`<!doctype html><html data-model-api-url="${publicEndpoint}" data-model-transport="direct"><title>CORS test</title><input id="api-key" value="DO_NOT_READ_FAKE_KEY"></html>`);});
let release,authorization,options=0;
const upstream=createServer((req,res)=>{
    if(req.method==='OPTIONS'){options++;res.writeHead(['/reject','/reject-auth','/reject-stream'].includes(req.url)?401:204);res.end();return;}
    authorization=req.headers.authorization;
    if(['/stream','/reject-stream'].includes(req.url)){res.writeHead(200,{'content-type':'text/event-stream'});res.write('data: first\n\n');release=()=>res.end('data: last\n\n');return;}
    const unauthorized=['/unauthorized','/reject-auth'].includes(req.url);
    res.writeHead(unauthorized?401:200,{'content-type':'application/json'});res.end(JSON.stringify({ok:!unauthorized}));
});
let context;
try{
    const origin=await listen(app),api=await listen(upstream),source=join(temporary,'extension');
    publicEndpoint=api+'/json';
    await cp(new URL('../../dist/cors-helper/chromium/',import.meta.url),source,{recursive:true});
    // Diagnostics are confined to this synthetic fixture, never distributed.
    const background=join(source,'background-debugger.js'),popup=join(source,'popup.js');
    await writeFile(background,(await readFile(background,'utf8')).replace("stage:error.stage??'setup'","stage:error.stage??'setup',errorDetail:error.message"));
    await writeFile(popup,(await readFile(popup,'utf8')).replace('new Error(result?.error??','new Error(result?.errorDetail??result?.error??'));
    const actions=join(source,'actions.js');
    await writeFile(actions,(await readFile(actions,'utf8')).replace("throw new Error('未能关闭预检修复，请在浏览器调试提示中手动取消后重试。')","throw new Error(result?.errorDetail??'No repair response')"));
    const manifest=JSON.parse(await readFile(join(source,'manifest.json'),'utf8'));
    // Only the isolated fixture gets pregranted hosts; shipped ZIP stays optional.
    manifest.host_permissions=['http://127.0.0.1/*'];await writeFile(join(source,'manifest.json'),JSON.stringify(manifest));
    context=await chromium.launchPersistentContext(join(temporary,'profile'),{channel:'chromium',headless:true,args:[`--disable-extensions-except=${source}`,`--load-extension=${source}`]});
    const worker=context.serviceWorkers()[0]??await context.waitForEvent('serviceworker');
    const page=await context.newPage();await page.goto(origin);
    const other=await context.newPage();await other.goto(origin);
    const extension=worker.url().split('/')[2],control=await context.newPage();
    const tabId=await worker.evaluate(async origin=>(await chrome.tabs.query({url:origin+'/*'}))[0].id,origin);
    const tabs=await worker.evaluate(async origin=>chrome.tabs.query({url:origin+'/*'}),origin);
    const pageIds=await page.evaluate(()=>location.href);assert.equal(pageIds,origin+'/');
    // The first created matching tab is the page under test.
    const targetId=Math.min(...tabs.map(tab=>tab.id));
    // Opening popup.html as a test tab cannot supply the toolbar's active-tab identity.
    // Override only that query; page scripting, permissions and DNR stay real.
    await control.addInitScript(({targetId,origin})=>{chrome.tabs.query=async()=>[{id:targetId,url:origin+'/'}];},{targetId,origin});
    await control.goto(`chrome-extension://${extension}/popup.html`);await control.setViewportSize({width:360,height:600});
    await control.waitForFunction(()=>!document.getElementById('enable').disabled);
    assert.equal(await control.locator('#endpoint').inputValue(),publicEndpoint);assert.equal(await control.locator('#manual').getAttribute('open'),null);
    assert.equal(await control.locator('#detected-url').innerText(),publicEndpoint);
    assert.ok(!await control.locator('body').innerText().then(text=>text.includes('DO_NOT_READ_FAKE_KEY')));
    assert.deepEqual(await control.evaluate(()=>({body:document.body.getBoundingClientRect().width,overflow:document.documentElement.scrollWidth>360})),{body:360,overflow:false});
    assert.equal(await control.locator('button').count(),1);
    assert.equal(await control.locator('#enable').innerText(),'启动');
    const fetchApi=(page,url)=>page.evaluate(async url=>{try{const res=await fetch(url,{method:'POST',credentials:'omit',headers:{'content-type':'application/json',authorization:'FAKE_TEST_KEY'},body:'{}'});return {status:res.status,text:await res.text()};}catch{return {blocked:true};}},url);
    assert.equal((await fetchApi(page,api+'/json')).blocked,true);
    const enable=endpoint=>control.evaluate(async({targetId,endpoint})=>{const {enableForTab}=await import('./actions.js');await enableForTab(targetId,endpoint);},{targetId,endpoint});
    const stop=async()=>{await control.locator('#enable').click();await control.waitForFunction(()=>document.getElementById('status').textContent==='未启动');};
    const repair=async endpoint=>{
        await page.evaluate(endpoint=>document.documentElement.dataset.modelApiUrl=endpoint,endpoint);
        await control.reload();await control.waitForFunction(()=>!document.getElementById('enable').disabled);
        control.once('dialog',dialog=>dialog.accept());await control.locator('#enable').click();
        try{await control.waitForFunction(()=>document.getElementById('status').textContent==='已启动 · 预检修复',{},{timeout:5000});}catch{throw new Error(await control.locator('#error').innerText());}
        assert.equal(await control.locator('#enable').innerText(),'停止');assert.equal(await control.locator('#manual').getAttribute('open'),null);
    };
    control.once('dialog',dialog=>dialog.dismiss());await control.locator('#enable').click();
    assert.equal((await fetchApi(page,api+'/json')).blocked,true);assert.equal(await control.locator('#enable').innerText(),'启动');
    await repair(api+'/json');
    assert.equal((await fetchApi(page,api+'/json')).status,200);assert.equal(authorization,'FAKE_TEST_KEY');
    await control.screenshot({path:new URL('../../dist/cors-helper/popup-0.3.1.png',import.meta.url).pathname});
    await stop();assert.equal((await fetchApi(page,api+'/json')).blocked,true);
    await worker.evaluate(()=>{globalThis.originalAttach=chrome.debugger.attach;chrome.debugger.attach=async()=>{throw new Error('fixture attach failure');};});
    control.once('dialog',dialog=>dialog.accept());await control.locator('#enable').click();
    await control.waitForFunction(()=>document.getElementById('error').textContent.includes('fixture attach failure')&&!document.getElementById('enable').disabled);
    assert.equal(await control.locator('#status').innerText(),'未启动');assert.equal((await fetchApi(page,api+'/json')).blocked,true);
    assert.equal((await worker.evaluate(()=>chrome.declarativeNetRequest.getSessionRules())).length,0);
    await worker.evaluate(()=>{chrome.debugger.attach=globalThis.originalAttach;});
    await control.locator('#manual summary').click();await control.locator('#endpoint').fill('not-a-url');await control.locator('#enable').click();await control.waitForFunction(()=>document.getElementById('error').textContent.includes('有效的完整 API'));
    assert.ok(!(await control.locator('#error').innerText()).includes('Invalid URL'));
    await enable(api+'/json');assert.equal((await fetchApi(page,api+'/json')).status,200);
    await page.evaluate(()=>document.documentElement.removeAttribute('data-model-api-url'));await control.reload();await control.waitForFunction(()=>!document.getElementById('enable').disabled);
    assert.notEqual(await control.locator('#manual').getAttribute('open'),null);assert.equal(await control.locator('#detected-url').isVisible(),false);
    await page.evaluate(()=>document.documentElement.dataset.modelTransport='relay');await control.reload();await control.waitForFunction(()=>document.getElementById('status').textContent==='无需启动');
    assert.equal(await control.locator('#enable').isDisabled(),false);await control.locator('#enable').click();await control.waitForFunction(()=>document.getElementById('enable').disabled);
    await page.evaluate(endpoint=>{document.documentElement.dataset.modelTransport='direct';document.documentElement.dataset.modelApiUrl=endpoint;},publicEndpoint);await control.reload();await control.waitForFunction(()=>!document.getElementById('enable').disabled);
    await enable(api+'/json');assert.equal((await fetchApi(other,api+'/json')).blocked,true);
    assert.equal((await fetchApi(page,api+'/other')).blocked,true);
    await enable(api+'/unauthorized');assert.equal((await fetchApi(page,api+'/unauthorized')).status,401);
    await enable(api+'/reject');assert.equal((await fetchApi(page,api+'/reject')).blocked,true);
    await control.reload();await control.waitForFunction(()=>!document.getElementById('enable').disabled);await stop();
    await repair(api+'/reject');
    const count=options;assert.equal((await fetchApi(page,api+'/reject')).status,200);assert.equal(options,count);assert.equal(authorization,'FAKE_TEST_KEY');
    assert.equal((await fetchApi(other,api+'/reject')).blocked,true);assert.equal((await fetchApi(page,api+'/reject-auth')).blocked,true);
    await stop();assert.equal((await fetchApi(page,api+'/reject')).blocked,true);
    await repair(api+'/reject-auth');assert.equal((await fetchApi(page,api+'/reject-auth')).status,401);
    await stop();assert.equal((await fetchApi(page,api+'/reject-auth')).blocked,true);
    await repair(api+'/reject-stream');
    await page.evaluate(url=>{window.first='';window.complete=false;window.streamTask=(async()=>{const res=await fetch(url,{method:'POST',credentials:'omit',headers:{authorization:'FAKE_TEST_KEY'}}),reader=res.body.getReader();let part;while(!(part=await reader.read()).done)window.first+=new TextDecoder().decode(part.value);window.complete=true;})();},api+'/reject-stream');
    await page.waitForFunction(()=>window.first.includes('first'));assert.equal(await page.evaluate(()=>window.complete),false);release();await page.evaluate(()=>window.streamTask);
    await worker.evaluate(async targetId=>chrome.debugger.detach({tabId:targetId}),targetId);assert.equal((await fetchApi(page,api+'/reject-stream')).blocked,true);
    await control.evaluate(async targetId=>{const {disableForTab}=await import('./actions.js');await disableForTab(targetId);},targetId);
    assert.equal((await fetchApi(page,api+'/stream')).blocked,true);
    await repair(api+'/reject');await page.goto(api+'/navigation');
    await control.waitForFunction(async targetId=>{
        const status=await chrome.runtime.sendMessage({action:'preflight-status',tabId:targetId});
        return status.ok&&!status.active&&!(await chrome.declarativeNetRequest.getSessionRules()).some(rule=>rule.id===targetId+1);
    },targetId);
    await page.goto(origin);assert.equal((await fetchApi(page,api+'/reject')).blocked,true);
    assert.ok(tabId>=0);console.log('PASS: real Chromium: explicit/cancelled preflight consent, OPTIONS 401 repair, exact URL/tab isolation, real POST 401 preserved, incremental SSE, debugger cancellation, full disable and navigation revocation; popup autofill/layout retained.');
}finally{
    await context?.close();app.closeAllConnections();upstream.closeAllConnections();await Promise.all([new Promise(resolve=>app.close(resolve)),new Promise(resolve=>upstream.close(resolve))]);
    await rm(temporary,{recursive:true,force:true});
}

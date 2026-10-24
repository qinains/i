import {test,expect} from './fixtures.mjs';
async function seed(page,files){await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.evaluate(async files=>{const fs=await import('/platform.js'),root=await fs.opfsOpen();for(const [path,text]of Object.entries(files))await fs.opfsWrite(root,path,new TextEncoder().encode(text));},files);}
async function enableScripts(page){page.once('dialog',dialog=>dialog.accept());await page.locator('#preview-scripts').check();}
test('preview has temporary local and session storage without access to workbench credentials',async({page})=>{
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    await seed(page,{'index.html':`<p id="result"></p><script>
    const absent=localStorage.getItem('preview-secret')===null;
    const count=Number(localStorage.getItem('count')||0)+1;
    localStorage.setItem('count',count);localStorage.theme='dark';sessionStorage.setItem('draft','temporary');
    let parentBlocked=false;try{parent.localStorage.getItem('preview-secret')}catch{parentBlocked=true}
    document.getElementById('result').textContent=[absent,count,localStorage.theme,sessionStorage.getItem('draft'),parentBlocked].join(':');
    </script>`});
    await page.evaluate(()=>localStorage.setItem('preview-secret','DUMMY_PARENT_SECRET'));
    await page.locator('#btn-preview').click();await enableScripts(page);
    await expect(page.frameLocator('#preview-frame').locator('#result')).toHaveText('true:1:dark:temporary:true');
    await expect(page.locator('#preview-frame')).toHaveAttribute('sandbox','allow-scripts');
    await page.locator('#refresh-preview').click();await expect(page.frameLocator('#preview-frame').locator('#result')).toHaveText('true:1:dark:temporary:true');
    expect(await page.evaluate(()=>({secret:localStorage.getItem('preview-secret'),count:localStorage.getItem('count'),draft:sessionStorage.getItem('draft')}))).toEqual({secret:'DUMMY_PARENT_SECRET',count:null,draft:null});
    expect(errors).toEqual([]);
});
test('preview skips hidden source highlighting and large source uses a single plain text node',async({page})=>{
    const html='<h1>Large file</h1>'+('<!-- padding -->\n'.repeat(10000));
    await seed(page,{'index.html':html});await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Large file');
    expect(await page.locator('#preview-source').textContent()).toBe('');
    await page.locator('#preview-mode').selectOption('source');await expect(page.locator('#preview-source')).toHaveText(html);await expect(page.locator('#preview-source span')).toHaveCount(0);
});
test('snapshot hash links stay inside the preview without navigating to the virtual workspace origin',async({page})=>{
    await seed(page,{'index.html':'<a href="#target">Jump</a><div style="height:1200px"></div><h2 id="target">Target</h2>'});
    const violations=[];page.on('console',message=>{if(message.text().includes('frame-src'))violations.push(message.text());});
    let hits=0;await page.route('https://workspace.invalid/**',route=>{hits++;return route.abort();});
    await page.locator('#btn-preview').click();const frame=page.frameLocator('#preview-frame');
    await frame.locator('a').click();await expect(frame.locator('#target')).toBeInViewport();
    expect(hits).toBe(0);expect(violations).toEqual([]);await expect(page.locator('#status')).toHaveText('Ready');
});
test('preview scripts are disabled by default and require explicit trust confirmation',async({page})=>{
    await seed(page,{'index.html':'<h1>Static</h1><script>document.querySelector("h1").textContent="Executed"</script>'});await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Static');await expect(page.locator('#preview-frame')).toHaveAttribute('sandbox','');
    page.once('dialog',dialog=>dialog.dismiss());await page.locator('#preview-scripts').click();await expect(page.locator('#preview-scripts')).not.toBeChecked();
    page.once('dialog',dialog=>dialog.accept());await page.locator('#preview-scripts').check();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Executed');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#preview-scripts')).not.toBeChecked();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Static');
});
test('opaque preview renders local HTML CSS classic JS and read-only fetch without exposing parent storage',async({page})=>{
    await seed(page,{'index.html':'<!doctype html><link rel="stylesheet" href="css/site.css"><h1>Preview</h1><p id="result"></p><script src="js/app.js"></script>',
        'css/site.css':'@import "./colors.css"; h1 {font-size:27px}', 'css/colors.css':'h1 {color:rgb(12,34,56)}',
        'js/app.js':`(async()=>{let storage;try{storage=parent.localStorage.getItem('preview-secret')}catch{storage='blocked'};const data=await fetch('/data.json?v=1').then(r=>r.json());document.querySelector('#result').textContent=storage+':'+data.answer;})();`,
        'data.json':'{"answer":42}'});
    await page.evaluate(()=>localStorage.setItem('preview-secret','DO_NOT_EXPOSE'));await page.locator('#btn-preview').click();await enableScripts(page);
    const frame=page.frameLocator('#preview-frame');await expect(frame.locator('#result')).toHaveText('blocked:42');await expect(frame.locator('h1')).toHaveCSS('color','rgb(12, 34, 56)');await expect(frame.locator('h1')).toHaveCSS('font-size','27px');
    expect(await page.locator('#preview-frame').getAttribute('sandbox')).toBe('allow-scripts');await expect(page.locator('#preview-status')).toContainText('snapshot');
});
test('preview has no network, parent scripting, form, worker or iframe privilege',async({page})=>{
    await seed(page,{'index.html':`<h1 id="result">running</h1><img src="data:image/png;base64,broken"><script>
    (async()=>{let parentAccess=false;try{parent.document.body.dataset.compromised='1';parentAccess=true}catch{};
    let network=false;try{await fetch('https://outside.invalid/leak')}catch{network=true};
    localStorage.setItem('leak','x');let storage=localStorage.getItem('leak')==='x';
    document.querySelector('#result').textContent=JSON.stringify({parentAccess,network,storage});
    const xhr=new XMLHttpRequest();xhr.open('GET','https://outside.invalid/xhr');try{xhr.send()}catch{};
    })();</script>`});
    let hits=0;await page.route('https://outside.invalid/**',route=>{hits++;return route.abort();});await page.locator('#btn-preview').click();await enableScripts(page);
    const frame=page.frameLocator('#preview-frame');await expect(frame.locator('#result')).toHaveText('{"parentAccess":false,"network":true,"storage":true}');expect(await page.locator('body').getAttribute('data-compromised')).toBeNull();expect(hits).toBe(0);
});
test('parent frame policy blocks script-driven preview navigation before any outbound request',async({page})=>{
    await seed(page,{'index.html':'<h1>Isolated</h1>'});let hits=0;await page.route('https://outside.invalid/**',route=>{hits++;return route.abort();});
    await page.evaluate(()=>{window.previewViolations=[];document.addEventListener('securitypolicyviolation',event=>window.previewViolations.push(event.violatedDirective));});
    await page.locator('#btn-preview').click();await enableScripts(page);await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Isolated');
    const frame=await page.locator('#preview-frame').contentFrame();await frame.locator('h1').evaluate(()=>{location.href='https://outside.invalid/navigation-leak';});
    await expect.poll(()=>page.evaluate(()=>window.previewViolations.includes('frame-src'))).toBe(true);expect(hits).toBe(0);await expect(page.locator('#status')).toHaveText('Ready');
});
test('missing and unsupported preview resources fail explicitly without navigating the app',async({page})=>{
    await seed(page,{'index.html':'<script type="module" src="app.js"></script>','app.js':"import 'react'"});await page.locator('#btn-preview').click();await enableScripts(page);await expect(page.locator('#preview-status')).toContainText('npm');await expect(page.locator('#preview-frame')).toBeHidden();await expect(page.locator('#status')).toHaveText('Ready');
});
test('local module graph supports cycles, reexports, dynamic imports and virtual import.meta.url',async({page})=>{
    await seed(page,{
        'index.html':'<h1>Static</h1><p id="result"></p><script type="module" src="assets/app.js"></script>',
        'assets/app.js':`import title,{count,bump} from './lib/index.js'; import {read} from './lib/cycle.js'; bump(); const lazy=await import('./lazy.js'); const data=await fetch(new URL('../data.json',import.meta.url)).then(r=>r.json()); let protectedStorage=false;try{parent.localStorage.getItem('secret')}catch{protectedStorage=true} document.querySelector('#result').textContent=[title,count,read(),lazy.default,data.answer,protectedStorage].join(':');`,
        'assets/lib/index.js':`export {default,count,bump} from './state.js';`,
        'assets/lib/state.js':`import {read} from './cycle.js'; export default 'modules'; export let count=0; export function bump(){count++}`,
        'assets/lib/cycle.js':`import {count} from './state.js'; export function read(){return count}`,
        'assets/lazy.js':`export default 'lazy';`,'data.json':'{"answer":42}',
    });
    await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Static');await enableScripts(page);
    await expect(page.frameLocator('#preview-frame').locator('#result')).toHaveText('modules:1:1:lazy:42:true');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#preview-scripts')).not.toBeChecked();await expect(page.frameLocator('#preview-frame').locator('#result')).toBeEmpty();await enableScripts(page);await expect(page.frameLocator('#preview-frame').locator('#result')).toHaveText('modules:1:1:lazy:42:true');
});
test('inline modules keep independent identity and share imported module singletons',async({page})=>{
    const inline=`import './shared.js'; document.querySelector('p').textContent+='inline';`;
    await seed(page,{'index.html':`<p></p><script type="module">${inline}</script><script type="module">${inline}</script>`,'shared.js':`document.querySelector('p').textContent+='shared';`});
    await page.locator('#btn-preview').click();await enableScripts(page);await expect(page.frameLocator('#preview-frame').locator('p')).toHaveText('sharedinlineinline');
});
test('disabled modules do not resolve dependencies; Chinese diagnostics identify missing local modules',async({page})=>{
    await seed(page,{'index.html':'<h1>Static only</h1><link rel="modulepreload" href="missing.js"><script type="module">import "./missing.js"</script>'});
    await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Static only');
    await page.evaluate(()=>document.documentElement.lang='zh-CN');await enableScripts(page);await expect(page.locator('#preview-status')).toContainText('找不到模块文件：/missing.js');await expect(page.locator('#preview-frame')).toBeHidden();
});
test('module preflight blocks external and computed imports before scripts or requests execute',async({page})=>{
    await seed(page,{'index.html':'<h1>Safe</h1>'});let hits=0;await page.route('https://outside.invalid/**',route=>{hits++;return route.abort();});await page.evaluate(()=>{document.documentElement.lang='zh-CN';window.moduleSignals=[];window.addEventListener('message',event=>window.moduleSignals.push(event.data));});
    for(const [code,message]of [[`import 'https://outside.invalid/leak.js'`,'外部 CDN'],[`import('./'+name)`,'计算式动态导入'],[`import 'react'`,'npm 包名'],[`import x from './a.js' with {type:'json'}`,'导入属性']]){
        await page.evaluate(async code=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'index.html',new TextEncoder().encode(`<h1>Safe</h1><script>parent.postMessage('unwanted','*')</script><script type="module">${code}</script>`));},code);
        await page.locator('#btn-preview').click();await enableScripts(page);await expect(page.locator('#preview-status')).toContainText(message);await expect(page.locator('#preview-frame')).toBeHidden();await page.locator('#close-preview').click();
    }
    expect(hits).toBe(0);
    expect(await page.evaluate(()=>window.moduleSignals)).toEqual([]);
});
test('preview snapshot stays unchanged until refreshed and closes when workspace changes',async({page})=>{
    await seed(page,{'index.html':'<h1>Before</h1>'});await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Before');
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'index.html',new TextEncoder().encode('<h1>After</h1>'));});await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Before');await page.locator('#refresh-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('After');
    page.once('dialog',dialog=>dialog.accept('Another'));await page.locator('#workspace-select').selectOption('__new__');await expect(page.locator('#preview-panel')).toBeHidden();
});
test('tmp preview cannot fetch normal workspace files',async({page})=>{
    await seed(page,{'secret.txt':'not in tmp preview','/tmp/index.html':'<p id="status"></p><script>fetch("/secret.txt").then(r=>document.querySelector("p").textContent=String(r.status))</script>'});
    await page.locator('#btn-preview').click();await page.locator('#preview-entry').fill('/tmp/index.html');await page.locator('#refresh-preview').click();await enableScripts(page);await expect(page.frameLocator('#preview-frame').locator('#status')).toHaveText('404');
});
test('preview refuses to snapshot while another window owns the workspace lock',async({page})=>{
    await seed(page,{'index.html':'<h1>Locked</h1>'});
    await page.evaluate(()=>{window.previewLock=new Promise(resolve=>navigator.locks.request('harness:workspace:default',async()=>{resolve();await new Promise(release=>window.releasePreviewLock=release);}));return window.previewLock;});
    await page.locator('#btn-preview').click();await expect(page.locator('#preview-status')).toContainText('another window');await expect(page.locator('#preview-frame')).toBeHidden();await page.evaluate(()=>window.releasePreviewLock());
});
test('reload of a missing preview shows its path without an opaque global error or creating files',async({page})=>{
    await seed(page,{'index.html':'<h1>Before removal</h1>'});await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Before removal');
    await expect.poll(()=>page.evaluate(async()=>{const {openStateStore}=await import('/state/store.js'),store=await openStateStore();const state=await store.load();store.close();return state.workspaces[0].fileView?.open;})).toBe(true);
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsRemove(await fs.opfsOpen(),'index.html');});await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#preview-status')).toContainText('/workspace/index.html');await expect(page.locator('#preview-status')).toContainText('not found');await expect(page.locator('#global-error')).toBeHidden();await expect(page.locator('#preview-frame')).toBeHidden();
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return fs.opfsList(await fs.opfsOpen(),'');})).toEqual([]);await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#preview-panel')).toBeHidden();
});
test('a truncated second model step never writes pending HTML or deletes an uploaded OPFS file',async({page})=>{
    await seed(page,{});await page.locator('#file-input').setInputFiles({name:'uploaded.md',mimeType:'text/markdown',buffer:Buffer.from('keep this upload')});await expect(page.locator('[data-file="uploaded.md"]')).toHaveCount(1);
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.route('**/mock/v1/chat/completions',route=>{const first=route.request().postDataJSON().messages.at(-1).role==='user',delta={role:'assistant',content:first?'Inspect files':'Will generate HTML',tool_calls:[{index:0,id:first?'list':'pending-write',type:'function',function:{name:first?'list':'write',arguments:JSON.stringify(first?{path:'/workspace'}:{path:'index.html',content:'not committed'})}}]};return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:first?'tool_calls':'length'}]})}\n\ndata: [DONE]\n\n`});});
    await page.locator('#input').fill('Build a website');await page.locator('#input').press('Enter');await expect(page.locator('#output')).toContainText('finish_reason=length');await expect(page.locator('#output')).toContainText('not executed');await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#approval-panel')).toBeHidden();await expect(page.locator('[data-file="index.html"]')).toHaveCount(0);
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('[data-file="uploaded.md"]')).toHaveCount(1);expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'uploaded.md',100));})).toBe('keep this upload');await page.locator('#btn-preview').click();await expect(page.locator('#preview-status')).toContainText('/workspace/index.html');await expect(page.locator('#global-error')).toBeHidden();
});

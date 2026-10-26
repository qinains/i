import {test,expect} from './fixtures.mjs';
async function ready(page,url='/') {
    await page.goto(url); await expect(page.locator('#status')).toHaveText('Ready');
    await page.evaluate(async () => {await navigator.serviceWorker.ready; if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener('controllerchange',resolve,{once:true}));});
}
test('PWA caches only static resources; offline WASM, OPFS and ZIP work at a subpath', async ({page,context}) => {
    await ready(page,'/sub/');
    await page.locator('#file-input').setInputFiles({name:'offline.txt',mimeType:'text/plain',buffer:Buffer.from('offline workspace')});
    await expect(page.locator('#file-list')).toContainText('offline.txt');
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('PRIVATE_KEY'); await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();
    const urls = await page.evaluate(async () => (await Promise.all((await caches.keys()).map(async name => (await (await caches.open(name)).keys()).map(r => r.url)))).flat());
    expect(urls.length).toBeGreaterThan(10); expect(urls.every(url => !/api\/|PRIVATE_KEY|test-pwa|mock/u.test(url))).toBeTruthy();
    await context.setOffline(true); await page.reload();
    await expect(page.locator('#status')).toHaveText('Ready'); await expect(page.locator('#file-list')).toContainText('offline.txt');
    await expect(page.locator('#pwa-status')).toContainText('Offline');
    const download = page.waitForEvent('download'); await page.locator('#btn-download').click(); await download;
    await page.locator('#input').fill('ask AI'); await page.locator('#input').press('Enter');
    await expect(page.locator('#global-error')).toContainText('Offline: the unsent message and attachments are retained.');
    await expect(page.locator('#input')).toHaveValue('ask AI');
});
test('offline subpath PWA previews persisted local ES modules without runtime downloads',async({page,context})=>{
    await ready(page,'/sub/');await page.evaluate(async()=>{const fs=await import('./platform.js'),root=await fs.opfsOpen();for(const [path,text]of Object.entries({'index.html':'<p></p><script type="module" src="js/app.js"></script>','js/app.js':`import {message} from './lib.js'; document.querySelector('p').textContent=message;`,'js/lib.js':`export const message='Offline modules';`}))await fs.opfsWrite(root,path,new TextEncoder().encode(text));});
    await context.setOffline(true);await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#btn-preview').click();page.once('dialog',dialog=>dialog.accept());await page.locator('#preview-scripts').check();await expect(page.frameLocator('#preview-frame').locator('p')).toHaveText('Offline modules');await expect(page.locator('#preview-frame')).toHaveAttribute('sandbox','allow-scripts');
});
test('updates fail safely on missing assets and wait for idle tabs before activation', async ({page,context}) => {
    await ready(page); const original = await page.evaluate(() => navigator.serviceWorker.controller.scriptURL);
    await page.locator('#file-input').setInputFiles({name:'keep.txt',mimeType:'text/plain',buffer:Buffer.from('retained through update')});
    await expect(page.locator('#file-list')).toContainText('keep.txt');await page.locator('#input').fill('My unsent draft');const owner=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    const other = await context.newPage(); await ready(other);
    try {
        await page.request.get('/test-pwa-state?version=broken&missing=1');
        await page.evaluate(async () => {const r = await navigator.serviceWorker.getRegistration(); await r.update(); if (r.installing) await new Promise(resolve => r.installing.addEventListener('statechange',function changed() {if (this.state === 'redundant' || this.state === 'installed') resolve();}));});
        expect(await page.evaluate(async () => !!(await navigator.serviceWorker.getRegistration()).waiting)).toBe(false);
        expect(await page.evaluate(() => navigator.serviceWorker.controller.scriptURL)).toBe(original);
        await page.request.get('/test-pwa-state?version=next');
        await page.evaluate(async () => {await (await navigator.serviceWorker.getRegistration()).update();});
        await expect(page.locator('#btn-update')).toBeVisible();
        await other.locator('#btn-new-session').click();await other.locator('#settings-btn').click();await other.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1'); await other.locator('#model').fill('fixture');await other.locator('#save-config').click();await other.locator('#close-settings').click();
        await other.locator('#input').fill('STREAM'); await other.locator('#input').press('Enter'); await expect(other.locator('#output')).toContainText('First visible');
        await page.setViewportSize({width:390,height:844});await page.locator('#btn-update').click(); await expect(page.locator('#pwa-status')).toContainText('another app tab is running a task');await expect(page.locator('#pwa-status')).toBeVisible();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
        await expect(page.locator('#btn-update')).toBeEnabled();await expect(page.locator('#input')).toBeEnabled();
        await expect(other.locator('#status')).toHaveText('Processing...');
        await other.locator('#btn-cancel').click(); await expect(other.locator('#status')).toHaveText('Ready');
        const reload = page.waitForEvent('load'); await page.locator('#btn-update').click(); await reload;
        await expect(page.locator('#status')).toHaveText('Ready');
        await page.locator(`[role=tab][data-session-id="${owner}"]`).click();await expect(page.locator('#file-list')).toContainText('keep.txt');await expect(page.locator('#input')).toHaveValue('My unsent draft');
        const names = await page.evaluate(() => caches.keys()); expect(names.some(name => name.endsWith('fixture-next'))).toBeTruthy();
    } finally {await page.request.get('/test-pwa-state'); await other.close();}
});
test('root update never locks another installed PWA sub-scope', async ({page,context}) => {
    await ready(page);
    const sub = await context.newPage(); await ready(sub,'/sub/');
    try {
        await page.request.get('/test-pwa-state?version=root-only');
        await page.evaluate(async () => {await (await navigator.serviceWorker.getRegistration()).update();});
        await expect(page.locator('#btn-update')).toBeVisible();
        const outcome = Promise.race([
            page.waitForEvent('load',{timeout:10000}).then(() => 'applied').catch(() => 'timeout'),
            page.locator('#pwa-status').filter({hasText:'Update postponed'}).waitFor({timeout:10000}).then(() => 'postponed').catch(() => 'timeout'),
        ]);
        await page.locator('#btn-update').click();
        const result = await outcome; expect(result).not.toBe('timeout');
        if (result === 'postponed') await expect(page.locator('#pwa-status')).toContainText('Do not clear site data');
        await expect(page.locator('#status')).toHaveText('Ready');
        await expect(sub.locator('#input')).toBeEnabled();
        expect(await sub.evaluate(() => navigator.serviceWorker.controller.scriptURL)).toContain('/sub/service-worker.js');
    } finally {await page.request.get('/test-pwa-state'); await sub.close();}
});
test('update waits for delayed page readiness beyond three seconds without losing a saved file',async({page,context})=>{
    await ready(page);const other=await context.newPage();await ready(other);
    try{
        await other.evaluate(()=>{const original=ServiceWorker.prototype.postMessage;ServiceWorker.prototype.postMessage=function(message,...args){if(message?.type==='pwa_idle'&&message.idle===true)setTimeout(()=>original.call(this,message,...args),4500);else original.call(this,message,...args);};});
        await page.locator('#file-input').setInputFiles({name:'slow-save.txt',mimeType:'text/plain',buffer:Buffer.from('still here')});await expect(page.locator('#file-list')).toContainText('slow-save.txt');
        await page.request.get('/test-pwa-state?version=slow-ready');await page.evaluate(async()=>{await (await navigator.serviceWorker.getRegistration()).update();});await expect(page.locator('#btn-update')).toBeVisible();
        const started=Date.now(),loaded=page.waitForEvent('load',{timeout:15000});await page.locator('#btn-update').click();await expect(page.locator('#btn-update')).toBeDisabled();await loaded;
        expect(Date.now()-started).toBeGreaterThanOrEqual(4000);await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#file-list')).toContainText('slow-save.txt');
    }finally{await page.request.get('/test-pwa-state');await other.close();}
});
test('PWA status follows the selected language immediately',async({page})=>{
    await ready(page);await page.evaluate(()=>window.dispatchEvent(new Event('online')));await expect(page.locator('#pwa-status')).toContainText('Online');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');
    await expect(page.locator('#pwa-status')).toContainText('已联网');await page.locator('#lang-select').selectOption('ja');await expect(page.locator('#pwa-status')).toContainText('オンライン');
});

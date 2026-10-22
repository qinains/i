import {test,expect} from './fixtures.mjs';
async function ready(page,url='/') {
    await page.goto(url); await expect(page.locator('#status')).toHaveText('Ready');
    await page.evaluate(async () => {await navigator.serviceWorker.ready; if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener('controllerchange',resolve,{once:true}));});
}
test('PWA caches only static resources; offline WASM, OPFS and ZIP work at a subpath', async ({page,context}) => {
    await ready(page,'/sub/');
    await page.locator('#file-input').setInputFiles({name:'offline.txt',mimeType:'text/plain',buffer:Buffer.from('offline workspace')});
    await expect(page.locator('#file-list')).toContainText('offline.txt');
    await page.locator('#api-key').fill('PRIVATE_KEY'); await page.locator('#remember-config').check();
    const urls = await page.evaluate(async () => (await Promise.all((await caches.keys()).map(async name => (await (await caches.open(name)).keys()).map(r => r.url)))).flat());
    expect(urls.length).toBeGreaterThan(10); expect(urls.every(url => !/api\/|PRIVATE_KEY|test-pwa|mock/u.test(url))).toBeTruthy();
    await context.setOffline(true); await page.reload();
    await expect(page.locator('#status')).toHaveText('Ready'); await expect(page.locator('#file-list')).toContainText('offline.txt');
    await expect(page.locator('#pwa-status')).toContainText('Offline');
    const download = page.waitForEvent('download'); await page.locator('#btn-download').click(); await download;
    await page.locator('#input').fill('ask AI'); await page.locator('#input').press('Enter');
    await expect(page.locator('#output')).toContainText('AI needs a network connection');
});
test('updates fail safely on missing assets and wait for idle tabs before activation', async ({page,context}) => {
    await ready(page); const original = await page.evaluate(() => navigator.serviceWorker.controller.scriptURL);
    const other = await context.newPage(); await ready(other);
    try {
        await page.request.get('/test-pwa-state?version=broken&missing=1');
        await page.evaluate(async () => {const r = await navigator.serviceWorker.getRegistration(); await r.update(); if (r.installing) await new Promise(resolve => r.installing.addEventListener('statechange',function changed() {if (this.state === 'redundant' || this.state === 'installed') resolve();}));});
        expect(await page.evaluate(async () => !!(await navigator.serviceWorker.getRegistration()).waiting)).toBe(false);
        expect(await page.evaluate(() => navigator.serviceWorker.controller.scriptURL)).toBe(original);
        await page.request.get('/test-pwa-state?version=next');
        await page.evaluate(async () => {await (await navigator.serviceWorker.getRegistration()).update();});
        await expect(page.locator('#btn-update')).toBeVisible();
        await other.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1'); await other.locator('#model').fill('fixture');
        await other.locator('#input').fill('STREAM'); await other.locator('#input').press('Enter'); await expect(other.locator('#output')).toContainText('First visible');
        await page.locator('#btn-update').click(); await expect(page.locator('#pwa-status')).toContainText('postponed');
        await expect(other.locator('#status')).toHaveText('Processing...');
        await other.locator('#btn-cancel').click(); await expect(other.locator('#status')).toHaveText('Ready');
        const reload = page.waitForEvent('load'); await page.locator('#btn-update').click(); await reload;
        await expect(page.locator('#status')).toHaveText('Ready');
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
        if (result === 'postponed') await expect(page.locator('#pwa-status')).toContainText('Close all app tabs');
        await expect(page.locator('#status')).toHaveText('Ready');
        await expect(sub.locator('#input')).toBeEnabled();
        expect(await sub.evaluate(() => navigator.serviceWorker.controller.scriptURL)).toContain('/sub/service-worker.js');
    } finally {await page.request.get('/test-pwa-state'); await sub.close();}
});

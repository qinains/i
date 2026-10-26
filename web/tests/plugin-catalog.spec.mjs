import {test,expect} from './fixtures.mjs';
import {pluginPackage} from './plugin-fixtures.mjs';
import {createHash} from 'node:crypto';
import {CATALOG_URL} from '../plugins/catalog.js';

async function fixture(page,{corrupt=false}={}){
    const bytes=Buffer.from(await pluginPackage()),sha256=createHash('sha256').update(bytes).digest('hex');
    const entry={id:'example.word-count',name:'Word count',version:'1.0.0',description:'<img src=x onerror="window.catalogXSS=1">',repository:'https://github.com/qinains/i',source_revision:'a'.repeat(40),license:'MIT',package:{url:`https://qinains.github.io/i-plugins/packages/${sha256}.hplugin.zip`,sha256,size:bytes.length},permissions:{workspace:'read',storage:false}};
    const requests=[];
    await page.route(CATALOG_URL,route=>{requests.push(route.request());return route.fulfill({contentType:'application/json',headers:{'access-control-allow-origin':'*'},body:JSON.stringify({schema_version:1,plugins:[entry]})});});
    await page.route(entry.package.url,route=>{requests.push(route.request());const body=Buffer.from(bytes);if(corrupt)body[body.length-1]^=1;return route.fulfill({contentType:'application/zip',headers:{'access-control-allow-origin':'*'},body});});
    return {entry,requests};
}
test('catalog inspect, install and workspace enable remain three separate user actions',async({page})=>{
    const {entry,requests}=await fixture(page);await page.goto('/?settings=plugins');await expect(page.locator('#plugin-catalog-load')).toBeEnabled();
    expect(requests).toHaveLength(0);await page.locator('#plugin-catalog-load').click();
    const item=page.locator(`[data-catalog-plugin="${entry.id}"]`);await expect(item).toContainText(entry.description);await expect(item.locator('img')).toHaveCount(0);
    expect(await page.evaluate(()=>window.catalogXSS)).toBeUndefined();await expect(page.locator(`[data-plugin-id="${entry.id}"]`)).toHaveCount(0);
    await item.locator('[data-catalog-inspect]').click();await expect(page.locator('#plugin-preview')).toBeVisible();await expect(page.locator('#plugin-preview')).toContainText('Unverified publisher');
    await expect(page.locator(`[data-plugin-id="${entry.id}"]`)).toHaveCount(0);await page.locator('#plugin-install').click();
    const installed=page.locator(`[data-plugin-id="${entry.id}"]`);await expect(installed).toContainText('Disabled');
    page.once('dialog',dialog=>dialog.dismiss());await installed.locator('[data-plugin-action=enable]').click();await expect(installed).toContainText('Disabled');
    page.once('dialog',dialog=>dialog.accept());await installed.locator('[data-plugin-action=enable]').click();await expect(installed).toContainText('Enabled');
    expect(requests).toHaveLength(2);for(const request of requests){expect(request.headers().authorization).toBeUndefined();expect(request.headers().cookie).toBeUndefined();expect(request.headers().referer).toBeUndefined();}
    await page.reload();await expect(installed).toContainText('Enabled');expect(requests).toHaveLength(2);
});
test('catalog tampering is rejected before installation',async({page})=>{
    const {entry}=await fixture(page,{corrupt:true});await page.goto('/?settings=plugins');await expect(page.locator('#plugin-catalog-load')).toBeEnabled();await page.locator('#plugin-catalog-load').click();
    await page.locator(`[data-catalog-inspect="${entry.id}"]`).click();await expect(page.locator('#plugin-status')).toContainText('digest mismatch');await expect(page.locator('#plugin-preview')).toBeHidden();await expect(page.locator(`[data-plugin-id="${entry.id}"]`)).toHaveCount(0);
});
test('live published registry installs both example packages without enabling them',async({page})=>{
    test.skip(process.env.LIVE_PLUGIN_CATALOG!=='1','Explicit opt-in: downloads public registry packages.');
    await page.goto('/?settings=plugins');await expect(page.locator('#plugin-catalog-load')).toBeEnabled();await page.locator('#plugin-catalog-load').click();
    for(const id of ['example.word-count','example.json-format']){
        await page.locator(`[data-catalog-inspect="${id}"]`).click();await expect(page.locator('#plugin-preview')).toContainText(id);await page.locator('#plugin-install').click();await expect(page.locator(`[data-plugin-id="${id}"]`)).toContainText('Disabled');
    }
});

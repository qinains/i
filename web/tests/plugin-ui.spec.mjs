import {test,expect} from './fixtures.mjs';
import {pluginPackage} from './plugin-fixtures.mjs';
test('plugin UI installs without enabling and restores workspace authorization after reload',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=plugins]').click();
    await page.locator('#plugin-file').setInputFiles({name:'count.hplugin.zip',mimeType:'application/zip',buffer:Buffer.from(await pluginPackage())});
    await expect(page.locator('#plugin-preview')).toContainText('Unverified publisher');await page.locator('#plugin-install').click();
    const card=page.locator('[data-plugin-id="example.word-count"]');await expect(card).toContainText('Disabled');
    page.once('dialog',d=>d.accept());await card.locator('[data-plugin-action=enable]').click();await expect(card).toContainText('Enabled');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=plugins]').click();await expect(card).toContainText('Enabled');
    page.once('dialog',d=>d.accept());await card.locator('[data-plugin-action=uninstall]').click();await expect(card).toHaveCount(0);
});
test('plugin UI Chinese controls fit phone and never render manifest names as HTML',async({page})=>{
    await page.setViewportSize({width:375,height:812});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.evaluate(()=>localStorage.setItem('i_lang','zh-CN'));await page.reload();await expect(page.locator('#status')).toHaveText('就绪');
    await page.locator('#settings-btn').click();await expect(page.locator('[data-settings-tab=plugins]')).toHaveText('插件');await page.locator('[data-settings-tab=plugins]').click();
    await page.locator('#plugin-file').setInputFiles({name:'count.hplugin.zip',mimeType:'application/zip',buffer:Buffer.from(await pluginPackage({name:'<img src=x onerror="window.pluginXSS=1">'}))});await expect(page.locator('#plugin-preview')).toContainText('未验证发布者');
    expect(await page.locator('#plugin-preview img').count()).toBe(0);expect(await page.evaluate(()=>window.pluginXSS)).toBeUndefined();
    expect(await page.evaluate(()=>document.querySelector('#settings-modal').scrollWidth<=document.querySelector('#settings-modal').clientWidth+1)).toBe(true);
});

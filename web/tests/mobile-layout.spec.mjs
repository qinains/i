import {test,expect} from './fixtures.mjs';

for(const width of [320,390,430])test(`mobile chrome preserves content space at ${width}px`,async({page},testInfo)=>{
    await page.setViewportSize({width,height:844});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#file-input').setInputFiles({name:'index.html',mimeType:'text/html',buffer:Buffer.from('<h1>Mobile preview</h1>')});
    expect((await page.locator('#context-bar').boundingBox()).height).toBeLessThanOrEqual(56);
    await expect(page.locator('#conversation-menu')).not.toHaveAttribute('open','');
    await page.locator('#conversation-menu>summary').click();await page.locator('#btn-preview').click();
    await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Mobile preview');
    expect((await page.locator('#preview-frame').boundingBox()).y-(await page.locator('#preview-panel').boundingBox()).y).toBeLessThanOrEqual(64);
    await page.screenshot({path:testInfo.outputPath(`preview-compact-${width}.png`)});
    await expect(page.locator('#preview-options')).not.toHaveAttribute('open','');
    await page.locator('#preview-options>summary').click();await expect(page.locator('#preview-scripts')).toBeVisible();await expect(page.locator('#preview-mode')).toBeVisible();await expect(page.locator('#preview-fullscreen')).toBeVisible();
    page.once('dialog',dialog=>dialog.dismiss());await page.locator('#preview-scripts').click();await expect(page.locator('#preview-scripts')).not.toBeChecked();
    await page.locator('#preview-mode').selectOption('source');await expect(page.locator('#preview-source')).toBeVisible();await page.locator('#preview-mode').selectOption('preview');await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Mobile preview');
    await page.keyboard.press('Escape');await expect(page.locator('#preview-options')).not.toHaveAttribute('open','');
    await page.locator('#refresh-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Mobile preview');
    await page.locator('#preview-entry').fill('missing.html');await page.locator('#refresh-preview').click();await expect(page.locator('#preview-status')).toContainText('not found');await expect(page.locator('#preview-status')).toBeVisible();
    await page.locator('#close-preview').click();
    await page.evaluate(()=>{document.getElementById('btn-update').hidden=false;document.getElementById('save-status').textContent='正在保存…';document.getElementById('model-name').textContent='a-very-long-model-name-that-must-not-wrap-the-footer';});
    expect((await page.locator('#status-bar').boundingBox()).height).toBeLessThanOrEqual(48);
    await expect(page.locator('#btn-update')).toBeVisible();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.screenshot({path:testInfo.outputPath(`mobile-${width}.png`)});
    await page.setViewportSize({width:1280,height:900});await expect(page.locator('#btn-preview')).toBeVisible();await page.locator('#btn-preview').click();await expect(page.locator('#preview-mode')).toBeVisible();await expect(page.locator('#preview-scripts')).toBeVisible();
});

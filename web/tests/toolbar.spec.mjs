import {test,expect} from './fixtures.mjs';
test('desktop files form a full-height left column beside the right workspace and footer',async({page},testInfo)=>{
    await page.setViewportSize({width:1280,height:900});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=appearance]').click();await page.locator('#theme-select').selectOption('light');await page.locator('#close-settings').click();
    await page.locator('#file-input').setInputFiles({name:'index.html',mimeType:'text/html',buffer:Buffer.from('<h1>Preview</h1>')});
    if(!await page.locator('#file-panel').isVisible())await page.locator('#btn-files').click();
    await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Preview');
    const files=await page.locator('#file-panel').boundingBox(),footer=await page.locator('#status-bar').boundingBox(),terminal=await page.locator('#terminal').boundingBox();
    expect(files.y+files.height).toBeGreaterThanOrEqual(footer.y+footer.height-1);
    expect(footer.x).toBeCloseTo(files.x+files.width,0);expect(terminal.x).toBeCloseTo(files.x+files.width,0);
    await expect(page.locator('#file-panel')).toHaveCSS('background-color','rgb(255, 255, 255)');
    await page.screenshot({path:testInfo.outputPath('full-height-files.png')});
    await page.locator('#close-files').click();expect((await page.locator('#status-bar').boundingBox()).x).toBe(0);
    await page.setViewportSize({width:390,height:844});await expect(page.locator('#preview-entry')).toBeVisible();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});
for(const width of [1280,900,390])test(`grouped toolbars stay compact and accessible at ${width}px`,async({page},testInfo)=>{
    await page.setViewportSize({width,height:900});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');await page.locator('#theme-select').selectOption('light');await page.locator('#close-settings').click();
    await page.locator('#file-input').setInputFiles({name:'index.html',mimeType:'text/html',buffer:Buffer.from('<h1>Preview</h1>')});
    if(width<=700)await page.locator('#conversation-menu>summary').click();
    await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Preview');
    await expect(page.locator('.context-actions .action-group')).toHaveCount(2);
    if(width<=700)await page.locator('#preview-options>summary').click();
    for(const id of ['refresh-preview','preview-fullscreen','close-preview']){
        const button=page.locator('#'+id);await expect(button.locator('svg')).toHaveCount(1);
        expect(await button.getAttribute('aria-label')).toBeTruthy();expect(await button.textContent()).toBe('');
        const box=await button.boundingBox();expect(box.width).toBeLessThanOrEqual(width<=700?44:34);
    }
    const path=await page.locator('#preview-entry').boundingBox();expect(path.width).toBeGreaterThan(120);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    expect(await page.locator('.preview-toolbar').evaluate(element=>element.scrollWidth<=element.clientWidth)).toBe(true);
    await expect(page.locator('#refresh-preview')).toHaveAttribute('aria-label','刷新预览');
    await expect(page.locator('#preview-fullscreen')).toHaveAttribute('aria-label','切换全屏');
    await page.screenshot({path:testInfo.outputPath(`toolbar-${width}.png`)});
    await page.locator('#preview-fullscreen').click();await expect(page.locator('#preview-panel')).toHaveClass('fullscreen');
    await page.locator('#preview-exit-fullscreen').click();await expect(page.locator('#preview-panel')).not.toHaveClass('fullscreen');
    await page.locator('#refresh-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Preview');
    await page.locator('#close-preview').click();await expect(page.locator('#preview-panel')).toBeHidden();
});

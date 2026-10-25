import {test,expect} from './fixtures.mjs';

for(const width of [1280,390,320])test(`fullscreen preview fills the viewport without app borders at ${width}px`,async({page},testInfo)=>{
    await page.setViewportSize({width,height:844});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#file-input').setInputFiles({name:'index.html',mimeType:'text/html',buffer:Buffer.from('<style>html,body{margin:0;background:#08b}</style><h1>Edge to edge</h1>')});
    if(width<=700)await page.locator('#conversation-menu>summary').click();
    await page.locator('#btn-preview').click();await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Edge to edge');
    if(width<=700)await page.locator('#preview-options>summary').click();
    await page.locator('#preview-fullscreen').click();
    const panel=page.locator('#preview-panel'),frame=page.locator('#preview-frame');
    await expect(panel).toHaveClass('fullscreen');
    for(const target of [panel,frame]){
        const box=await target.boundingBox();expect(box.x).toBe(0);expect(box.y).toBe(0);expect(box.width).toBe(width);expect(box.height).toBe(844);
        await expect(target).toHaveCSS('border-left-width','0px');await expect(target).toHaveCSS('border-right-width','0px');
    }
    await expect(page.locator('.preview-toolbar')).toBeHidden();await expect(page.locator('#preview-options')).toBeHidden();
    const exit=page.locator('#preview-exit-fullscreen');await expect(exit).toBeVisible();
    await page.screenshot({path:testInfo.outputPath(`fullscreen-${width}.png`)});
    await page.setViewportSize({width,height:650});expect((await frame.boundingBox()).height).toBe(650);
    await exit.click();await expect(panel).not.toHaveClass('fullscreen');await expect(page.locator('.preview-toolbar')).toBeVisible();
    if(width<=700&&!await page.locator('#preview-options').evaluate(element=>element.open))await page.locator('#preview-options>summary').click();
    await page.locator('#preview-fullscreen').click();await page.keyboard.press('Escape');await expect(panel).not.toHaveClass('fullscreen');
    await page.locator('#close-preview').click();await expect(panel).toBeHidden();
});

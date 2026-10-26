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
    const start=await exit.boundingBox();
    await page.mouse.move(start.x+22,start.y+22);await page.mouse.down();
    await page.mouse.move(80,450,{steps:12});await page.mouse.up();
    await expect(panel).toHaveClass('fullscreen');
    const moved=await exit.boundingBox();expect(moved.x).toBeLessThan(start.x-40);expect(moved.y).toBeGreaterThan(350);
    await expect(exit).toHaveCSS('touch-action','none');
    await page.screenshot({path:testInfo.outputPath(`fullscreen-${width}.png`)});
    await page.setViewportSize({width,height:350});expect((await frame.boundingBox()).height).toBe(350);
    const clamped=await exit.boundingBox();expect(clamped.y).toBeGreaterThanOrEqual(12);expect(clamped.y+clamped.height).toBeLessThanOrEqual(338);
    await page.setViewportSize({width,height:650});
    await exit.click();await expect(panel).not.toHaveClass('fullscreen');await expect(page.locator('.preview-toolbar')).toBeVisible();
    if(width<=700&&!await page.locator('#preview-options').evaluate(element=>element.open))await page.locator('#preview-options>summary').click();
    await page.locator('#preview-fullscreen').click();await page.keyboard.press('Escape');await expect(panel).not.toHaveClass('fullscreen');
    await page.locator('#close-preview').click();await expect(panel).toBeHidden();
});

test.describe('touch fullscreen control',()=>{
    test.use({hasTouch:true});
    test('pointer cancellation does not exit; keyboard movement and mobile tap still work',async({page})=>{
        await page.setViewportSize({width:390,height:844});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
        await page.locator('#file-input').setInputFiles({name:'index.html',mimeType:'text/html',buffer:Buffer.from('<h1>Touch preview</h1>')});
        await page.locator('#conversation-menu>summary').click();await page.locator('#btn-preview').click();
        await expect(page.frameLocator('#preview-frame').locator('h1')).toHaveText('Touch preview');
        await page.locator('#preview-options>summary').click();await page.locator('#preview-fullscreen').click();
        const exit=page.locator('#preview-exit-fullscreen'),panel=page.locator('#preview-panel');
        const initial=await exit.boundingBox();await page.mouse.move(initial.x+22,initial.y+22);await page.mouse.down();
        await exit.evaluate(button=>{
            for(const type of ['pointermove','pointercancel']){
                button.dispatchEvent(new PointerEvent(type,{pointerId:1,pointerType:'mouse',isPrimary:true,button:0,clientX:90,clientY:400,bubbles:true}));
            }
        });
        await page.mouse.up();
        await expect(panel).toHaveClass('fullscreen');
        const box=await exit.boundingBox();expect(box.x).toBeLessThan(100);expect(box.y).toBeGreaterThan(350);
        await exit.focus();await page.keyboard.press('ArrowDown');expect((await exit.boundingBox()).y).toBe(box.y+10);
        await page.keyboard.press('Enter');await expect(panel).not.toHaveClass('fullscreen');
        if(!await page.locator('#preview-options').evaluate(element=>element.open))await page.locator('#preview-options>summary').click();
        await page.locator('#preview-fullscreen').click();
        const restored=await exit.boundingBox();expect(restored.x).toBe(box.x);
        await page.touchscreen.tap(restored.x+22,restored.y+22);await expect(panel).not.toHaveClass('fullscreen');
    });
});

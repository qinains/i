import {test,expect} from './fixtures.mjs';
test('RTL selects reserve arrow space on the left on desktop and mobile',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('ar');
    for(const width of [1280,390]){
        await page.setViewportSize({width,height:844});
        for(const id of ['lang-select','theme-select','workspace-select','execution-mode','profile-select']){
            const style=await page.locator(`#${id}`).evaluate(element=>{const s=getComputedStyle(element);return {left:parseFloat(s.paddingLeft),right:parseFloat(s.paddingRight),position:s.backgroundPosition,direction:s.direction};});
            expect(style.direction).toBe('rtl');expect(style.left,`${id} arrow gutter`).toBeGreaterThan(style.right);
            expect(style.position).not.toContain('100%');
        }
    }
    await page.locator('#lang-select').selectOption('en');
    const padding=await page.locator('#workspace-select').evaluate(element=>{const s=getComputedStyle(element);return [parseFloat(s.paddingLeft),parseFloat(s.paddingRight)];});expect(padding[1]).toBeGreaterThan(padding[0]);
});
test('automatic, light and dark themes update live and persist without saving credentials',async({page},testInfo)=>{
    await page.emulateMedia({colorScheme:'light'});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('html')).toHaveAttribute('data-theme','light');
    await page.emulateMedia({colorScheme:'dark'});await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
    await page.locator('#settings-btn').click();await page.locator('#api-key').fill('UNSAVED_THEME_TEST_KEY');await page.locator('[data-settings-tab=appearance]').click();
    await page.locator('#theme-select').selectOption('light');await expect(page.locator('html')).toHaveAttribute('data-theme','light');
    await page.emulateMedia({colorScheme:'light'});await page.emulateMedia({colorScheme:'dark'});await expect(page.locator('html')).toHaveAttribute('data-theme','light');
    expect(await page.evaluate(()=>getComputedStyle(document.body).backgroundColor)).toBe('rgb(243, 245, 248)');
    await page.screenshot({path:testInfo.outputPath('theme-light.png')});
    expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('UNSAVED_THEME_TEST_KEY');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('html')).toHaveAttribute('data-theme','light');await expect(page.locator('#theme-select')).toHaveValue('light');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=appearance]').click();await page.locator('#theme-select').selectOption('dark');
    expect(await page.evaluate(()=>getComputedStyle(document.body).backgroundColor)).toBe('rgb(16, 19, 24)');
    await page.screenshot({path:testInfo.outputPath('theme-dark.png')});
    await page.locator('#theme-select').selectOption('auto');await page.emulateMedia({colorScheme:'light'});await expect(page.locator('html')).toHaveAttribute('data-theme','light');
});

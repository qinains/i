import {test,expect} from './fixtures.mjs';
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

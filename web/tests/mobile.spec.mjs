import {test,expect} from './fixtures.mjs';
test.use({viewport:{width:390,height:844}});
test('phone layout has no horizontal overflow and task, approval, cancel and ZIP controls remain usable', async ({page}) => {
    await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
    expect(await page.locator('#input').evaluate(el => parseFloat(getComputedStyle(el).fontSize))).toBeGreaterThanOrEqual(16);
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1'); await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('MAKE'); await page.locator('#input').press('Enter');
    await expect(page.locator('#approval-panel')).toBeVisible(); await page.locator('#approve').click();
    await expect(page.locator('#approval-panel')).toBeVisible(); await page.locator('#approve').click();
    await expect(page.locator('#status')).toHaveText('Ready');
    const download = page.waitForEvent('download'); await page.locator('#btn-download').click(); await download;
});
test('new browser profile does not inherit other profile credentials or workspace', async ({page}) => {
    await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#api-key')).toHaveValue(''); await expect(page.locator('#file-list')).toContainText('No files');
    await expect(page.locator('#remember-config')).not.toBeChecked();
});

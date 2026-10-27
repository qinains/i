import {test,expect} from './fixtures.mjs';
test('CORS helper packages download from localized installation instructions',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');await page.locator('[data-settings-tab=connection]').click();await page.locator('#cors-help summary').click();
    await expect(page.locator('#cors-help')).toContainText('不修复真实 API 鉴权或 TLS 错误');await expect(page.locator('#cors-help')).toContainText('Xcode');
    const links=page.locator('#cors-help a[download]');await expect(links).toHaveCount(3);
    for(const link of await links.all()){
        const waiting=page.waitForEvent('download');await link.click();const download=await waiting;
        expect(download.suggestedFilename()).toMatch(/^harness-cors-helper-(chromium|firefox|safari)-0\.3\.1\.zip$/u);expect(await download.failure()).toBeNull();
    }
});

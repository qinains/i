import {test,expect} from './fixtures.mjs';

test('sending without a URL keeps the draft and offers a focused settings action',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();
    await page.locator('#input').fill('Keep this unsent task');await page.locator('#btn-send').click();
    await expect(page.locator('#global-error')).toContainText('API URL');await expect(page.locator('#global-error')).not.toContainText("Failed to construct");
    await expect(page.locator('#input')).toHaveValue('Keep this unsent task');
    await page.locator('#open-connection-settings').click();await expect(page.locator('#settings-modal')).toBeVisible();
    await expect(page.locator('[data-settings-panel=connection]')).toBeVisible();await expect(page.locator('#base-url')).toBeFocused();
    await expect(page.locator('#config-status')).toContainText('API key');
    await page.locator('#base-url').fill('https://api.example');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();
    await expect(page.locator('#global-error')).toBeHidden();
});

test('invalid URL in settings shows translated guidance without exposing URL credentials',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await page.locator('#settings-btn').click();
    await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');await page.locator('[data-settings-tab=connection]').click();
    await page.locator('#base-url').fill('https://user:URL_SECRET@api.example');await page.locator('#model').fill('fixture');await page.locator('#test-connection').click();
    await expect(page.locator('#config-status')).toContainText('API URL');await expect(page.locator('#config-status')).toContainText('密钥');
    await expect(page.locator('#base-url')).toBeFocused();await expect(page.locator('#base-url')).toHaveAttribute('aria-invalid','true');
    await expect(page.locator('#global-error')).not.toContainText('URL_SECRET');
});

test('resuming a saved queue without configuration offers settings and retains the queued message',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.evaluate(()=>{const original=window.fetch;window.fetch=(url,options)=>String(url).includes('/mock/v1/chat/completions')?new Promise((resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new DOMException('Aborted','AbortError')),{once:true})):original(url,options);});
    await page.locator('#input').fill('first');await page.locator('#btn-send').click();await expect(page.locator('#input')).toBeEnabled();
    await page.locator('#input').fill('Keep this queued task');await page.locator('#btn-send').click();await page.locator('#btn-cancel').click();await expect(page.locator('#status')).toHaveText('Ready');
    await page.evaluate(async()=>{const {clearConfiguration}=await import('/config.js');clearConfiguration(localStorage);});await page.reload();await expect(page.locator('#input')).toBeEnabled();
    await page.locator('#queue-resume').click();await expect(page.locator('#open-connection-settings')).toBeVisible();
    await expect(page.locator('.pending-message')).toContainText('Keep this queued task');
});

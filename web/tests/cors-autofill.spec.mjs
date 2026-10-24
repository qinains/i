import {test,expect} from './fixtures.mjs';

test('only the applied non-secret API URL is published for extension autofill',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await expect(page.locator('html')).not.toHaveAttribute('data-model-api-url');
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('https://api.example/v1');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('AUTOFILL_FAKE_KEY');
    await expect(page.locator('html')).not.toHaveAttribute('data-model-api-url');await page.locator('#save-config').click();
    await expect(page.locator('html')).toHaveAttribute('data-model-api-url','https://api.example/v1/chat/completions');await expect(page.locator('html')).toHaveAttribute('data-model-transport','direct');
    await page.locator('#base-url').fill('https://unsaved.example');await expect(page.locator('html')).toHaveAttribute('data-model-api-url','https://api.example/v1/chat/completions');
    await page.locator('#transport').selectOption('relay');await page.locator('#relay-token').fill('AUTOFILL_FAKE_RELAY');await page.locator('#save-config').click();
    await expect(page.locator('html')).toHaveAttribute('data-model-transport','relay');expect(await page.locator('html').evaluate(el=>JSON.stringify(el.dataset))).not.toContain('AUTOFILL_FAKE');
    await page.locator('[data-settings-tab=advanced]').click();await page.locator('details.danger summary').click();page.once('dialog',dialog=>dialog.accept());await page.locator('#clear-config').click();await expect(page.locator('html')).not.toHaveAttribute('data-model-api-url');
});

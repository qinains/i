import {test,expect} from './fixtures.mjs';
test('explicit own relay streams through real server without CORS or buffering, preserving 401', async ({page}) => {
    await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('https://api.example/v1'); await page.locator('#model').fill('fixture');
    await page.locator('[data-settings-tab=connection]').click();await page.locator('#transport').selectOption('relay'); await page.locator('#relay-token').fill('fixture-relay-token-'.repeat(3));await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('RELAY'); await page.locator('#input').press('Enter');
    await expect(page.locator('#output')).toContainText('Relay first'); await expect(page.locator('#status')).toHaveText('Processing...');
    await page.request.get('/release-stream'); await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=connection]').click();await page.locator('#api-key').fill('RETURN_401');page.once('dialog',d=>d.accept());await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('RELAY'); await page.locator('#input').press('Enter');
    await expect(page.locator('#output')).toContainText('API 401'); await expect(page.locator('#output')).not.toContainText('RETURN_401');
    await expect(page.locator('#file-list')).toContainText('No files');
});

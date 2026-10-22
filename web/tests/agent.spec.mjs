import {test, expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';
async function configure(page) {
    await page.goto('/');
    await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');
    await page.locator('#model').fill('fixture');
    await page.locator('#api-key').fill('TEST_SECRET_NOT_PERSISTED');
}
async function submit(page, task) { await page.locator('#input').fill(task); await page.locator('#input').press('Enter'); }
test('real WASM model tools edit OPFS and export a real ZIP', async ({page}) => {
    await configure(page);
    await submit(page,'MAKE');
    await expect(page.locator('#approval-panel')).toBeVisible(); await page.locator('#approve').click();
    await expect(page.locator('#approval-panel')).toBeVisible(); await page.locator('#approve').click();
    await expect(page.locator('#output')).toContainText('All done. ZIP ready.');
    await expect(page.locator('#file-list')).toContainText('nested/结果.txt');
    const content = await page.evaluate(async () => {
        const fs = await import('/platform.js'); return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'nested/结果.txt',100));
    });
    expect(content).toBe('world\n');
    const downloadPromise = page.waitForEvent('download'); await page.locator('#downloads a').click();
    const download = await downloadPromise; expect(download.suggestedFilename()).toBe('workspace.zip');
    const bytes = await readFile(await download.path()); expect([...bytes.subarray(0,4)]).toEqual([80,75,3,4]);
    expect(bytes.includes(Buffer.from('world\n'))).toBeTruthy();
    const storage = await page.evaluate(() => JSON.stringify(localStorage)); expect(storage).not.toContain('TEST_SECRET');
    await page.reload(); await expect(page.locator('#file-list')).toContainText('nested/结果.txt');
});
test('denial and forged shell calls never mutate workspace', async ({page}) => {
    await configure(page); await submit(page,'DENY');
    await expect(page.locator('#approval-panel')).toBeVisible(); await page.locator('#deny').click();
    await expect(page.locator('#output')).toContainText('Tool refused as expected.');
    await submit(page,'SHELL'); await expect(page.locator('#output')).toContainText('unsupported tool in browser: shell');
    await expect(page.locator('#file-list')).toContainText('No files');
});
test('cancelled approval and late network responses never write files', async ({page}) => {
    await configure(page); await submit(page,'MAKE');
    await expect(page.locator('#approval-panel')).toBeVisible(); await page.locator('#btn-cancel').click();
    await expect(page.locator('#status')).toHaveText('Ready');
    await submit(page,'WAIT'); await page.locator('#btn-cancel').click();
    await page.waitForTimeout(3200);
    await expect(page.locator('#file-list')).toContainText('No files');
});
test('truncated batch and composition Enter cannot execute tools', async ({page}) => {
    await configure(page);
    await page.locator('#input').fill('MAKE');
    await page.locator('#input').dispatchEvent('keydown', {key:'Enter',isComposing:true});
    await expect(page.locator('#status')).toHaveText('Ready');
    await submit(page,'TRUNCATE'); await expect(page.locator('#output')).toContainText('incomplete tool response');
    await expect(page.locator('#file-list')).toContainText('No files');
});

import {test, expect} from './fixtures.mjs';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
async function configure(page) {
    await page.goto('/');
    await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');
    await page.locator('#model').fill('fixture');
    await page.locator('#api-key').fill('TEST_SECRET_NOT_PERSISTED');
}
async function submit(page, task) { await page.locator('#input').fill(task); await page.locator('#input').press('Enter'); }
test('stream displays before server release without duplicate final output; cancel isolates next request', async ({page}) => {
    await configure(page); await submit(page,'STREAM');
    await expect(page.locator('#output')).toContainText('First visible');
    await expect(page.locator('#status')).toHaveText('Processing...');
    await expect(page.locator('#btn-clear')).toBeDisabled();
    await page.request.get('/release-stream');
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('.line.assistant')).toHaveCount(1);
    await expect(page.locator('.line.assistant')).toHaveText('First visible then done');
    await submit(page,'STREAM'); await expect(page.locator('.line.assistant')).toHaveCount(2);
    await page.locator('#btn-cancel').click();
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#file-list')).toContainText('No files');
    await expect(page.locator('#output')).toContainText('[incomplete]');
});
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
    expect(execFileSync('unzip',['-p',await download.path(),'nested/结果.txt']).toString()).toBe('world\n');
    const storage = await page.evaluate(() => JSON.stringify(localStorage)); expect(storage).not.toContain('TEST_SECRET');
    await page.reload(); await expect(page.locator('#file-list')).toContainText('nested/结果.txt');
});
test('binary upload and button download share the persistent workspace', async ({page}) => {
    await configure(page);
    await page.locator('#file-input').setInputFiles({name:'binary.bin',mimeType:'application/octet-stream',buffer:Buffer.from([0,255,10])});
    await expect(page.locator('#output')).toContainText('Uploaded: binary.bin');
    const promise = page.waitForEvent('download'); await page.locator('#btn-download').click();
    const download = await promise;
    expect([...execFileSync('unzip',['-p',await download.path(),'binary.bin'])]).toEqual([0,255,10]);
    await page.reload(); await expect(page.locator('#file-list')).toContainText('binary.bin (3 bytes)');
});
test('unavailable OPFS fails explicitly without enabling fake file tools', async ({page}) => {
    await page.addInitScript(() => Object.defineProperty(navigator,'storage',{value:{}}));
    await page.goto('/');
    await expect(page.locator('#status')).toHaveText('Unavailable');
    await expect(page.locator('#output')).toContainText('OPFS requires HTTPS or localhost');
    await expect(page.locator('#btn-upload')).toBeDisabled();
});
test('CORS failures report an error without writing files or leaking credentials', async ({page}) => {
    await configure(page);
    await page.locator('#base-url').fill('http://127.0.0.1:4174/mock/v1');
    await submit(page,'MAKE');
    await expect(page.locator('#output')).toContainText('Network request failed');
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#file-list')).toContainText('No files');
    await expect(page.locator('#output')).not.toContainText('TEST_SECRET_NOT_PERSISTED');
});
test('opt-in restores the entire harness configuration and clearing removes it', async ({page}) => {
    await configure(page);
    await page.locator('#max-steps').fill('12');
    await page.locator('#max-tokens').fill('4000');
    await page.locator('#use-max-tokens').uncheck();
    await page.locator('#lang-select').selectOption('zh-CN');
    await page.locator('#remember-config').check();
    await expect(page.locator('#config-status')).toHaveText('Configuration saved in this browser.');
    await page.reload(); await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#base-url')).toHaveValue('http://127.0.0.1:4173/mock/v1');
    await expect(page.locator('#model')).toHaveValue('fixture');
    await expect(page.locator('#api-key')).toHaveValue('TEST_SECRET_NOT_PERSISTED');
    await expect(page.locator('#max-steps')).toHaveValue('12');
    await expect(page.locator('#max-tokens')).toHaveValue('4000');
    await expect(page.locator('#use-max-tokens')).not.toBeChecked();
    await expect(page.locator('#lang-select')).toHaveValue('zh-CN');
    await expect(page.locator('#remember-config')).toBeChecked();
    await page.locator('#clear-config').click();
    await expect(page.locator('#api-key')).toHaveValue('');
    const saved = await page.evaluate(() => localStorage.getItem('i_harness_config'));
    expect(saved).toBeNull();
    await page.reload(); await expect(page.locator('#api-key')).toHaveValue('');
});
test('disabling configuration persistence deletes the key without clearing OPFS', async ({page}) => {
    await configure(page);
    await page.locator('#remember-config').check();
    await page.locator('#remember-config').uncheck();
    expect(await page.evaluate(() => localStorage.getItem('i_harness_config'))).toBeNull();
    await expect(page.locator('#api-key')).toHaveValue('TEST_SECRET_NOT_PERSISTED');
    await page.reload(); await expect(page.locator('#api-key')).toHaveValue('');
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
    await page.locator('#input').dispatchEvent('keydown', {key:'Enter',isComposing:false,keyCode:229});
    await expect(page.locator('#status')).toHaveText('Ready');
    await submit(page,'TRUNCATE'); await expect(page.locator('#output')).toContainText('incomplete tool response');
    await expect(page.locator('#file-list')).toContainText('No files');
});

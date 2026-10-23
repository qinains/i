import {test,expect} from './fixtures.mjs';
test('reload restores a saved uninitialized conversation and draft without requiring a system message',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#input').fill('unsent draft');await expect(page.locator('#save-status')).toHaveText('Saved');
    const before=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    // Reproduce the durable checkpoint between create() and lazy agent initialization.
    await page.evaluate(async id=>{const {openStateStore}=await import('/state/store.js'),store=await openStateStore();try{const state=await store.load(),record=state.sessions.find(record=>record.id===id);record.history=[];await store.commitSession(record,record.revision);}finally{store.close();}},before);
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#global-error')).toBeHidden();await expect(page.locator('#input')).toHaveValue('unsent draft');
    await expect(page.locator('[role=tab][aria-selected=true]')).toHaveAttribute('data-session-id',before);
});
test('session save failure still allows a read-only ZIP backup',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#file-input').setInputFiles({name:'backup.txt',mimeType:'text/plain',buffer:Buffer.from('keep me')});await expect(page.locator('#file-list')).toContainText('backup.txt');await expect(page.locator('#save-status')).toHaveText('Saved');
    await page.evaluate(()=>{const original=IDBObjectStore.prototype.put;IDBObjectStore.prototype.put=function(value,...args){if(this.name==='sessions'){this.transaction.abort();throw new DOMException('Injected quota failure','QuotaExceededError');}return original.call(this,value,...args);};});
    await page.locator('#input').fill('unsaved draft');await expect(page.locator('#save-status')).toHaveText('Save failed');const download=page.waitForEvent('download');await page.locator('#btn-download').click();expect((await download).suggestedFilename()).toMatch(/\.zip$/);
});
async function configure(page){await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#remember-config').check();await page.locator('#close-settings').click();}
test('refresh during approval restores a valid interrupted batch without executing or repeating it',async({page})=>{
    await configure(page);await page.locator('#input').fill('MAKE');await page.locator('#input').press('Enter');await expect(page.locator('#approval-panel')).toBeVisible();
    let requests=0;page.on('request',r=>{if(r.url().endsWith('/chat/completions'))requests++;});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#output')).toContainText('interrupted');
    await expect(page.locator('#approval-panel')).toBeHidden();await expect(page.locator('#file-list')).toContainText('No files');expect(requests).toBe(0);
    let history;
    await page.route('**/mock/v1/chat/completions',async route=>{history=route.request().postDataJSON().messages;await route.fulfill({contentType:'text/event-stream',body:'data: {"choices":[{"index":0,"delta":{"content":"Recovered without replay"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'});});
    await page.locator('#input').fill('Continue');await page.locator('#input').press('Enter');await expect(page.locator('#output')).toContainText('Recovered without replay');
    expect(history.some(m=>m.role==='tool'&&m.content.includes('not executed'))).toBe(true);
});
test('file written before result checkpoint failure is recovered as unknown, never written again',async({page})=>{
    await configure(page);
    await page.evaluate(()=>{const original=IDBObjectStore.prototype.put;IDBObjectStore.prototype.put=function(value,...args){if(this.name==='sessions'&&value.journal?.calls?.some(c=>c.status==='finished')){this.transaction.abort();throw new DOMException('Injected quota failure','QuotaExceededError');}return original.call(this,value,...args);};});
    await page.locator('#input').fill('MAKE');await page.locator('#input').press('Enter');await expect(page.locator('#approval-panel')).toBeVisible();await page.locator('#approve').click();
    await expect(page.locator('#save-status')).toHaveText('Save failed');await expect(page.locator('[data-file="nested/结果.txt"]')).toHaveCount(1);
    let requests=0;page.on('request',r=>{if(r.url().endsWith('/chat/completions'))requests++;});await page.reload();await expect(page.locator('#status')).toHaveText('Ready');expect(requests).toBe(0);
    const state=await page.evaluate(async()=>{const {openStateStore}=await import('/state/store.js');const store=await openStateStore();const result=await store.load();store.close();return result.sessions[0];});
    expect(state.history.some(m=>m.role==='tool'&&m.content.includes('outcome unknown'))).toBe(true);
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'nested/结果.txt',100));})).toBe('hello\n');
});
test('two windows reject conflicting session saves instead of overwriting a newer draft',async({page,context})=>{
    await page.goto('/');await expect(page.locator('#save-status')).toHaveText('Saved');const second=await context.newPage();await second.goto('/');await expect(second.locator('#status')).toHaveText('Ready');
    await second.locator('#input').fill('newer draft');await expect(second.locator('#save-status')).toHaveText('Saved');
    await page.locator('#input').fill('stale draft');await expect(page.locator('#global-error')).toContainText('another window');
    await second.reload();await expect(second.locator('#input')).toHaveValue('newer draft');await second.close();
});

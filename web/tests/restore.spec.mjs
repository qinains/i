import {test,expect} from './fixtures.mjs';
async function setup(page){
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');
    await page.locator('[data-settings-tab=connection]').click();await page.locator('#stream').uncheck();await page.locator('#save-config').click();await page.locator('#close-settings').click();
}
test('refresh restores selected session, real model history, transcript, draft and persistent files',async({page})=>{
    await setup(page);await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=connection]').click();await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();const histories=[];
    await page.route('**/mock/v1/chat/completions',async route=>{
        histories.push(route.request().postDataJSON().messages);
        await route.fulfill({contentType:'application/json',body:JSON.stringify({choices:[{finish_reason:'stop',message:{role:'assistant',content:'Saved reply'}}]})});
    });
    await page.locator('#input').fill('FIRST');await page.locator('#input').press('Enter');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#btn-new-session').click();await expect(page.locator('[role=tab]')).toHaveCount(2);
    await page.locator('#input').fill('SECOND');await page.locator('#input').press('Enter');await expect(page.locator('#status')).toHaveText('Ready');
    expect(histories[1].some(m=>m.content==='FIRST')).toBe(false);
    await page.locator('#input').fill('unsent draft');await expect(page.locator('#save-status')).toHaveText('Saved');
    const selected=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('[role=tab][aria-selected=true]')).toHaveAttribute('data-session-id',selected);
    await expect(page.locator('#output')).toContainText('SECOND');await expect(page.locator('#input')).toHaveValue('unsent draft');
    await page.locator('#input').fill('CONTINUE');await page.locator('#input').press('Enter');await expect(page.locator('#status')).toHaveText('Ready');
    expect(histories[2].some(m=>m.content==='SECOND')).toBe(true);expect(histories[2].some(m=>m.content==='Saved reply')).toBe(true);
});
test('workspaces isolate files while sessions within a workspace share them',async({page})=>{
    await setup(page);await page.locator('#file-input').setInputFiles({name:'isolated.txt',mimeType:'text/plain',buffer:Buffer.from('workspace A')});
    await expect(page.locator('#file-list')).toContainText('isolated.txt');
    await page.locator('#btn-new-session').click();await expect(page.locator('#file-list')).toContainText('isolated.txt');
    page.once('dialog',dialog=>dialog.accept('B'));await page.locator('#workspace-select').selectOption('__new__');
    await expect(page.locator('#workspace-select option:checked')).toHaveText('B');await expect(page.locator('#file-list')).toContainText('No files');
    await page.locator('#file-input').setInputFiles({name:'isolated.txt',mimeType:'text/plain',buffer:Buffer.from('workspace B')});
    await expect(page.locator('#file-list')).toContainText('isolated.txt');await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#workspace-select option:checked')).toHaveText('B');
    await page.locator('#workspace-select').selectOption('default');await expect(page.locator('#file-list')).toContainText('isolated.txt');
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'isolated.txt',100));})).toBe('workspace A');
});
test('untrusted model output stays text and interrupted stream reload never retries',async({page})=>{
    await setup(page);
    await page.route('**/mock/v1/chat/completions',route=>route.fulfill({contentType:'application/json',body:JSON.stringify({choices:[{finish_reason:'stop',message:{role:'assistant',content:'<img src=x onerror="window.injected=true">'}}]})}));
    await page.locator('#input').fill('XSS');await page.locator('#input').press('Enter');await expect(page.locator('#status')).toHaveText('Ready');
    expect(await page.evaluate(()=>window.injected===true)).toBe(false);await expect(page.locator('#output img')).toHaveCount(0);
    await page.unroute('**/mock/v1/chat/completions');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=connection]').click();await page.locator('#stream').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('STREAM');await page.locator('#input').press('Enter');await expect(page.locator('#output')).toContainText('First visible');
    await expect(page.locator('#save-status')).toHaveText('Saved');
    let calls=0;page.on('request',r=>{if(r.url().endsWith('/chat/completions'))calls++;});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#output')).toContainText('First visible');
    await expect(page.locator('#output')).toContainText(/interrupted|incomplete/);expect(calls).toBe(0);
});

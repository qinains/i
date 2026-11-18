import {test,expect} from './fixtures.mjs';
import {readFile} from 'node:fs/promises';
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
test('a one-time IndexedDB abort keeps the draft and the next save succeeds',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.evaluate(()=>{
        const put=IDBObjectStore.prototype.put;let abortNext=true;
        IDBObjectStore.prototype.put=function(value,...args){
            if(this.name==='sessions'&&abortNext){abortNext=false;this.transaction.abort();}
            else return put.call(this,value,...args);
        };
    });
    await page.locator('#input').fill('retain on failure');await expect(page.locator('#save-status')).toHaveText('Save failed');
    await expect(page.locator('#export-unsent')).toBeVisible();
    await page.locator('#input').fill('retain after retry');await expect(page.locator('#save-status')).toHaveText('Saved');
    await expect(page.locator('#export-unsent')).toBeHidden();
    await page.reload();await expect(page.locator('#input')).toHaveValue('retain after retry');
});
test('actual WASM reconstructs all journal statuses and validates results',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const history=await page.evaluate(async()=>{
        const {BrowserAgent}=await import('/pkg/i.js');
        const base=[{role:'system',content:'test'}],calls=['a','b','c'].map(id=>({id,type:'function',function:{name:'view_image',arguments:'{}'}}));
        const image={type:'image_url',image_url:{url:'data:image/png;base64,iVBORw0KGgo='}};
        const journal={assistant:{role:'assistant',tool_calls:calls},calls:[{id:'a',status:'finished',result:{output:'Image attached.',images:[image]}},{id:'b',status:'started'},{id:'c',status:'pending'}]};
        return JSON.parse(BrowserAgent.recover_history(JSON.stringify(base),JSON.stringify(journal)));
    });
    expect(history.map(message=>message.role)).toEqual(['system','assistant','tool','tool','tool','user']);
    expect(history[2].content).toBe('Image attached.');expect(history[3].content).toContain('outcome unknown');expect(history[4].content).toContain('not executed');
    expect(history[5].content[0].text).toBe('Image from tool "a"');
});
async function configure(page){await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();}
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
test('two windows reject conflicting saves and the stale window can export its unsent draft and attachment',async({page,context})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#save-status')).toHaveText('Saved');const second=await context.newPage();await second.goto('/');await expect(second.locator('#status')).toHaveText('Ready');
    await second.locator('#input').fill('newer draft');await expect(second.locator('#save-status')).toHaveText('Saved');
    let modelRequests=0;page.on('request',request=>{if(request.url().includes('/chat/completions'))modelRequests++;});
    const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=';
    await page.locator('#input').evaluate((input,png)=>{
        const data=new DataTransfer();data.items.add(new File([Uint8Array.from(atob(png),c=>c.charCodeAt(0))],'image.png',{type:'image/png'}));
        const event=new ClipboardEvent('paste',{bubbles:true,cancelable:true});Object.defineProperty(event,'clipboardData',{value:data});input.dispatchEvent(event);
    },png);
    await expect(page.locator('#input')).toHaveValue('[Image #1]');await page.locator('#input').fill('[Image #1] stale draft');
    await expect(page.locator('#global-error')).toContainText('another window');
    const download=page.waitForEvent('download');await page.locator('#export-unsent').click();
    const artifact=await download;expect(artifact.suggestedFilename()).toBe('i-unsent-draft.json');
    expect(JSON.parse(await readFile(await artifact.path(),'utf8'))).toEqual({
        format:'i-unsent-session',version:1,draft:'[Image #1] stale draft',
        attachments:[{kind:'image',token:'[Image #1]',mime:'image/png',base64:png}],queue:[],
    });
    expect(modelRequests).toBe(0);
    await second.reload();await expect(second.locator('#input')).toHaveValue('newer draft');await second.close();
});

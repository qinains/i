import {test,expect} from './fixtures.mjs';
import {execFileSync} from 'node:child_process';
async function start(page){await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');}
test('tmp mount is separate, shared by sessions, excluded from default ZIP and cleared on reload',async({page})=>{
    await start(page);
    await page.evaluate(async()=>{const fs=await import('/platform.js');const root=await fs.opfsOpen();await fs.opfsWrite(root,'tmp/keep.txt',new TextEncoder().encode('persistent'));});
    await page.locator('#btn-files').click();await page.locator('#file-actions > summary').click();await page.locator('#upload-target').selectOption('/tmp/');await page.locator('#file-input').setInputFiles({name:'scratch.txt',mimeType:'text/plain',buffer:Buffer.from('scratch')});await expect(page.locator('[data-file="/tmp/scratch.txt"]')).toHaveCount(1);
    await page.locator('#btn-new-session').click();await expect(page.locator('[role=tab]')).toHaveCount(2);await expect(page.locator('[data-file="/tmp/scratch.txt"]')).toHaveCount(1);
    const download=page.waitForEvent('download');await page.locator('#btn-download').click();const zip=await download;const listing=execFileSync('unzip',['-Z1',await zip.path()]).toString();expect(listing).toContain('tmp/keep.txt');expect(listing).not.toContain('scratch.txt');
    const tmpDownload=page.waitForEvent('download');await page.locator('#btn-download-tmp').click();const tmpZip=await tmpDownload;expect(execFileSync('unzip',['-p',await tmpZip.path(),'tmp/scratch.txt']).toString()).toBe('scratch');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#file-list')).not.toContainText('scratch.txt');await expect(page.locator('[data-file="tmp/keep.txt"]')).toHaveCount(1);
});
test('temporary workspace files and conversations never enter persistent storage',async({page})=>{
    await start(page);page.once('dialog',dialog=>dialog.accept('Private scratch'));await page.locator('#workspace-select').selectOption('__scratch__');await expect(page.locator('#workspace-select option:checked')).toContainText('Temporary');
    await page.locator('#file-input').setInputFiles({name:'only-memory.txt',mimeType:'text/plain',buffer:Buffer.from('memory')});await expect(page.locator('#file-list')).toContainText('only-memory.txt');await page.locator('#input').fill('private unsent draft');await expect(page.locator('#save-status')).toContainText('Temporary');
    const saved=await page.evaluate(async()=>{const {openStateStore}=await import('/state/store.js');const store=await openStateStore();const state=await store.load();store.close();return JSON.stringify(state);});expect(saved).not.toContain('Private scratch');expect(saved).not.toContain('private unsent');expect(saved).not.toContain('temp-');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#workspace-select')).not.toContainText('Private scratch');await expect(page.locator('#file-list')).not.toContainText('only-memory.txt');
});
test('clearing tmp does not clear workspace files',async({page})=>{
    await start(page);await page.evaluate(async()=>{const fs=await import('/platform.js');const root=await fs.opfsOpen();await fs.opfsWrite(root,'keep.txt',new Uint8Array([1]));await fs.opfsWrite(root,'/tmp/drop.txt',new Uint8Array([2]));});
    await page.locator('#btn-files').click();await page.locator('#file-actions > summary').click();page.once('dialog',dialog=>dialog.accept());await page.locator('#btn-clear-tmp').click();await expect(page.locator('#file-list')).toContainText('keep.txt');await expect(page.locator('#file-list')).not.toContainText('drop.txt');
});
test('real WASM tools write the tmp mount and mount-colliding ZIP paths are rejected',async({page})=>{
    await start(page);await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.route('**/mock/v1/chat/completions',route=>{const messages=route.request().postDataJSON().messages;const message=messages.at(-1).role==='user'?{role:'assistant',content:null,tool_calls:[{index:0,id:'tmp-write',type:'function',function:{name:'write',arguments:'{"path":"/tmp/from-model.txt","content":"from wasm"}'}}]}:{role:'assistant',content:'tmp complete'};return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta:message,finish_reason:message.tool_calls?'tool_calls':'stop'}]})}\n\ndata: [DONE]\n\n`});});
    await page.locator('#input').fill('Write a scratch file');await page.locator('#btn-send').click();await expect(page.locator('#approval-panel')).toBeVisible();await page.locator('#approve').click();await expect(page.locator('#output')).toContainText('tmp complete');await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('[data-file="/tmp/from-model.txt"]')).toHaveCount(1);
    const result=await page.evaluate(async()=>{const {BrowserAgent}=await import('/pkg/i.js');const agent=await BrowserAgent.create(()=>{},()=>true,()=>{});await agent.upload('tmp/from-model.txt',new Uint8Array([1]));try{await agent.export_zip('{"paths":["tmp/from-model.txt","/tmp/from-model.txt"]}');return 'unexpected success';}catch(error){return String(error);}finally{agent.free();}});expect(result).toContain('collide');
});

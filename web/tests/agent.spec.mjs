import {test, expect} from './fixtures.mjs';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
async function configure(page) {
    await page.goto('/');
    await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');
    await page.locator('#model').fill('fixture');
    await page.locator('#api-key').fill('TEST_SECRET_NOT_PERSISTED');
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
}
async function submit(page, task) { await page.locator('#input').fill(task); await page.locator('#input').press('Enter'); }
test('completed reasoning stays separate, redacted and collapsible after reload',async({page})=>{
    await configure(page);
    await page.route('**/mock/v1/chat/completions',route=>route.fulfill({contentType:'text/event-stream',body:
        [{reasoning_content:'Inspect TEST_SECRET_NOT_PERSISTED safely.'},{content:'Final answer.'}].map(delta=>`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:null}]})}\n\n`).join('')
        +`data: ${JSON.stringify({choices:[{index:0,delta:{},finish_reason:'stop'}]})}\n\ndata: [DONE]\n\n`}));
    await submit(page,'Show reasoning');await expect(page.locator('#status')).toHaveText('Ready');
    const thinking=page.locator('details.reasoning');await expect(thinking.locator('summary')).toHaveText('Thinking');
    await expect(thinking.locator('pre')).toBeHidden();await thinking.locator('summary').click();
    await expect(thinking.locator('pre')).toHaveText('Inspect [redacted] safely.');
    await expect(page.locator('.line.assistant')).toHaveText('Final answer.Copy');
    await expect(page.locator('#save-status')).toHaveText('Saved');
    await page.screenshot({path:test.info().outputPath('expanded-reasoning.png')});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    await expect(thinking.locator('pre')).toBeVisible();await expect(thinking.locator('pre')).toHaveText('Inspect [redacted] safely.');
    await thinking.locator('summary').click();await expect(thinking.locator('pre')).toBeHidden();
});
test('reasoning and pending tool parameters show progress; length failure never runs or retries tools',async({page})=>{
    await configure(page);
    await page.evaluate(()=>{
        const original=window.fetch;window.progressRequests=0;
        window.fetch=(url,options)=>{
            if(!String(url).includes('/mock/v1/chat/completions'))return original(url,options);
            window.progressRequests++;
            return Promise.resolve(new Response(new ReadableStream({start(controller){window.progressStream=controller;}}),{headers:{'content-type':'text/event-stream'}}));
        };
    });
    await submit(page,'Test pending tool progress');
    await expect(page.locator('#stream-status')).toContainText('Waiting for model');
    const frame=async(delta,finish_reason=null)=>page.evaluate(({delta,finish_reason})=>{
        window.progressStream.enqueue(new TextEncoder().encode(`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason}]})}\n\n`));
    },{delta,finish_reason});
    await frame({reasoning_content:'Readable reasoning'});
    await expect(page.locator('#stream-status')).toContainText('Model is reasoning');
    const reasoning=page.locator('details.reasoning');await expect(reasoning).toHaveCount(1);
    await expect(reasoning.locator('pre')).toBeHidden();await reasoning.locator('summary').click();
    await expect(reasoning.locator('pre')).toHaveText('Readable reasoning');
    await frame({reasoning_content:' <b>not HTML</b>'});
    await expect(reasoning.locator('pre')).toBeVisible();await expect(reasoning.locator('pre')).toContainText('<b>not HTML</b>');await expect(reasoning.locator('b')).toHaveCount(0);
    await frame({tool_calls:[{index:0,id:'pending',type:'function',function:{name:'write',arguments:'{"path":"never.txt","content":"PRIVATE'}}]});
    await expect(page.locator('#stream-status')).toContainText('Preparing tool: write');
    await expect(page.locator('#stream-status')).toContainText('not executed');
    await expect(page.locator('#approval-panel')).toBeHidden();
    await expect(page.locator('#output')).not.toContainText('PRIVATE');
    await frame({},'length');
    await expect(page.locator('#output')).toContainText('finish_reason=length');
    await expect(reasoning.locator('summary')).toHaveText('Thinking');
    await expect(page.locator('.line.assistant')).toHaveCount(0);
    await expect(page.locator('#output')).toContainText('Pending tool calls were not executed');
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#stream-status')).toBeHidden();
    await expect(page.locator('#file-list')).toContainText('No files');
    expect(await page.evaluate(()=>window.progressRequests)).toBe(1);
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#stream-status')).toBeHidden();
    await expect(page.locator('#output')).toContainText('finish_reason=length');
    await expect(page.locator('details.reasoning pre')).toBeVisible();
    await expect(page.locator('details.reasoning pre')).toContainText('Readable reasoning');
});
test('stream displays before server release without duplicate final output; cancel isolates next request', async ({page}) => {
    await configure(page); await submit(page,'STREAM');
    await expect(page.locator('#output')).toContainText('First visible');
    await expect(page.locator('#status')).toHaveText('Processing...');
    await expect(page.locator('#btn-clear')).toBeDisabled();
    await page.request.get('/release-stream');
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('.line.assistant')).toHaveCount(1);
    await expect(page.locator('.line.assistant')).toContainText('First visible then done');
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
    await expect(page.locator('[data-file="nested/结果.txt"]')).toHaveCount(1);
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
    await page.reload(); await expect(page.locator('[data-file="nested/结果.txt"]')).toHaveCount(1);
});
test('binary upload and button download share the persistent workspace', async ({page}) => {
    await configure(page);
    await page.locator('#file-input').setInputFiles({name:'binary.bin',mimeType:'application/octet-stream',buffer:Buffer.from([0,255,10])});
    await expect(page.locator('#output')).toContainText('Uploaded: binary.bin');
    const promise = page.waitForEvent('download'); await page.locator('#btn-download').click();
    const download = await promise;
    expect([...execFileSync('unzip',['-p',await download.path(),'binary.bin'])]).toEqual([0,255,10]);
    await page.reload();await page.locator('[data-file="binary.bin"]').click();await expect(page.locator('#file-details')).toHaveText('binary.bin · 3 bytes');
});
test('uploaded and AI-generated files share real OPFS across reload while only absolute tmp is discarded',async({page})=>{
    await configure(page);
    await page.locator('#file-input').setInputFiles({name:'uploaded.txt',mimeType:'text/plain',buffer:Buffer.from('manual upload')});
    await expect(page.locator('[data-file="uploaded.txt"]')).toHaveCount(1);
    const writes=[['/workspace/generated/index.html','<h1>AI generated</h1>'],['generated/assets/app.js','export const persistent=true;'],['tmp/ordinary.txt','relative tmp is persistent'],['/tmp/scratch.txt','absolute tmp is temporary']];
    await page.route('**/mock/v1/chat/completions',route=>{
        const first=route.request().postDataJSON().messages.at(-1).role==='user';
        const delta={role:'assistant',content:first?null:'Files written.',...(first?{tool_calls:writes.map(([path,content],index)=>({index,id:`persist-${index}`,type:'function',function:{name:'write',arguments:JSON.stringify({path,content})}}))}:{})};
        return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,finish_reason:first?'tool_calls':'stop',delta}]})}\n\ndata: [DONE]\n\n`});
    });
    await submit(page,'Generate persistent files alongside the manual upload');
    for(const [path]of writes){await expect(page.locator('#approval-panel')).toBeVisible();await expect(page.locator('#approval-description')).toContainText(path);await page.locator('#approve').click();}
    await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#output')).toContainText('Files written.');
    const disk=()=>page.evaluate(async()=>{
        const root=await (await navigator.storage.getDirectory()).getDirectoryHandle('workspace');
        const read=async path=>{const parts=path.split('/');let dir=root;for(const part of parts.slice(0,-1))dir=await dir.getDirectoryHandle(part);return (await (await dir.getFileHandle(parts.at(-1))).getFile()).text();};
        return Promise.all(['uploaded.txt','generated/index.html','generated/assets/app.js','tmp/ordinary.txt'].map(read));
    });
    const expected=['manual upload',...writes.slice(0,3).map(([,content])=>content)];
    expect(await disk()).toEqual(expected);await expect(page.locator('[data-file="/tmp/scratch.txt"]')).toHaveCount(1);
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');expect(await disk()).toEqual(expected);
    await expect(page.locator('[data-file="/tmp/scratch.txt"]')).toHaveCount(0);await expect(page.locator('#file-list details[data-path="/workspace"]')).toHaveAttribute('data-file-count','4');
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
    await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('http://127.0.0.1:4174/mock/v1');
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await submit(page,'MAKE');
    await expect(page.locator('#output')).toContainText('Network request failed');
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#file-list')).toContainText('No files');
    await expect(page.locator('#output')).not.toContainText('TEST_SECRET_NOT_PERSISTED');
});
test('opt-in restores the entire harness configuration and clearing removes it', async ({page}) => {
    await configure(page);
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=connection]').click();
    await expect(page.locator('#max-steps')).toHaveCount(0);
    await page.locator('#max-tokens').fill('4000');
    await page.locator('#use-max-tokens').uncheck();
    await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');
    await page.locator('[data-settings-tab=connection]').click();
    await page.locator('#remember-config').check();
    await page.locator('#save-config').click();
    await expect(page.locator('#config-status')).toHaveText('连接配置已保存在此浏览器。');
    await page.reload(); await expect(page.locator('#status')).toHaveText('就绪');
    await expect(page.locator('#base-url')).toHaveValue('http://127.0.0.1:4173/mock/v1');
    await expect(page.locator('#model')).toHaveValue('fixture');
    await expect(page.locator('#api-key')).toHaveValue('TEST_SECRET_NOT_PERSISTED');
    await expect(page.locator('#max-steps')).toHaveCount(0);
    await expect(page.locator('#max-tokens')).toHaveValue('4000');
    await expect(page.locator('#use-max-tokens')).not.toBeChecked();
    await expect(page.locator('#lang-select')).toHaveValue('zh-CN');
    await expect(page.locator('#remember-config')).toBeChecked();
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=advanced]').click();await page.locator('details.danger summary').click();page.once('dialog',d=>d.accept());await page.locator('#clear-config').click();
    await expect(page.locator('#api-key')).toHaveValue('');
    const saved = await page.evaluate(() => localStorage.getItem('i_harness_config'));
    expect(saved).toBeNull();
    await page.reload(); await expect(page.locator('#api-key')).toHaveValue('');
});
test('disabling configuration persistence deletes the key without clearing OPFS', async ({page}) => {
    await configure(page);
    await page.locator('#settings-btn').click();
    await page.locator('#remember-config').check();
    await page.locator('#save-config').click();
    await page.locator('#remember-config').uncheck();
    expect(await page.evaluate(() => localStorage.getItem('i_harness_config'))).toBeNull();
    expect(await page.evaluate(() => localStorage.getItem('i_harness_profiles'))).toBeNull();
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
    await configure(page); await expect(page.locator('#file-list')).toContainText('No files');await submit(page,'MAKE');
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
    await page.locator('#input').fill('TRUNCATE');await page.locator('#btn-send').click();await expect(page.locator('#output')).toContainText('finish_reason=length');
    await expect(page.locator('#file-list')).toContainText('No files');
});

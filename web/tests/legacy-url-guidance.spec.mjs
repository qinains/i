import {test,expect} from './fixtures.mjs';
const original="Error: Failed to construct 'URL': Invalid URL";

test('restored legacy URL error offers settings without changing stored history or files',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await page.locator('#input').fill('Retain my draft');await expect(page.locator('#save-status')).toHaveText('Saved');
    await page.evaluate(async text=>{
        const {opfsOpenWorkspace,opfsWrite}=await import('/platform.js');await opfsWrite(await opfsOpenWorkspace('default'),'kept.txt',new TextEncoder().encode('retained'));
        const {openStateStore}=await import('/state/store.js'),store=await openStateStore();try{const record=(await store.load()).sessions[0];record.transcript.push({type:'error',text});await store.commitSession(record,record.revision);}finally{store.close();}
    },original);
    await page.reload();await expect(page.locator('#input')).toBeEnabled();
    await expect(page.locator('#output')).not.toContainText(original);await expect(page.locator('#output')).toContainText('Historical error');
    await page.locator('#output [data-open-connection-settings]').click();await expect(page.locator('#base-url')).toBeFocused();await expect(page.locator('#settings-modal')).toBeVisible();
    expect(await page.evaluate(async()=>{const {openStateStore}=await import('/state/store.js'),store=await openStateStore();try{return (await store.load()).sessions[0].transcript.filter(line=>line.type==='error').map(line=>line.text);}finally{store.close();}})).toEqual([original]);
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpenWorkspace('default'),'kept.txt',1000));})).toBe('retained');
    await expect(page.locator('#input')).toHaveValue('Retain my draft');
});

test('assistant text is not interpreted as a legacy connection failure',async({page})=>{
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();
    const result=await page.evaluate(async text=>{const {renderMessage}=await import('/render.js');const el=renderMessage({type:'assistant',text},{onConfigure:()=>{}});return {text:el.textContent,actions:el.querySelectorAll('[data-open-connection-settings]').length};},original);
    expect(result.text).toContain(original);expect(result.actions).toBe(0);
});

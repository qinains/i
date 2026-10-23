import {test,expect} from './fixtures.mjs';
test('corrupt transcript is isolated without losing workspace files or blocking startup',async({page})=>{
    await page.goto('/');await expect(page.locator('#save-status')).toHaveText('Saved');
    await page.evaluate(async()=>{const {openStateStore}=await import('/state/store.js');const store=await openStateStore();const {sessions}=await store.load();const record=sessions[0];record.transcript=[null];await new Promise((resolve,reject)=>{const request=indexedDB.open('harness_state');request.onsuccess=()=>{const db=request.result;const tx=db.transaction('sessions','readwrite');tx.objectStore('sessions').put(record);tx.oncomplete=()=>{db.close();resolve();};tx.onerror=()=>reject(tx.error);};});store.close();});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#global-error')).toContainText('invalid');await expect(page.locator('#input')).toBeEnabled();
});
test('deleted connection requires explicit attachment before old history is sent',async({page})=>{
    await page.route('**/mock/v1/chat/completions',route=>route.fulfill({contentType:'text/event-stream',body:'data: {"choices":[{"index":0,"delta":{"content":"fixture reply"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'}));
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#remember-config').check();await page.locator('#close-settings').click();
    await page.locator('#input').fill('HELLO');await page.locator('#input').press('Enter');await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#output')).toContainText('fixture');
    await page.locator('#settings-btn').click();page.once('dialog',d=>d.accept());await page.locator('#delete-profile').click();await page.locator('#close-settings').click();
    let requests=0;page.on('request',r=>{if(r.url().endsWith('/chat/completions'))requests++;});await page.locator('#input').fill('Continue');await page.locator('#input').press('Enter');await expect(page.locator('#output')).toContainText('another or deleted connection');expect(requests).toBe(0);
    await page.locator('#settings-btn').click();page.once('dialog',d=>d.accept());await page.locator('#attach-connection').click();await page.locator('#close-settings').click();await page.locator('#input').fill('Continue');await page.locator('#input').press('Enter');await expect.poll(()=>requests).toBe(1);
});
test('legacy configuration and malformed saved profiles never block local workspace startup',async({page})=>{
    await page.addInitScript(()=>localStorage.setItem('i_harness_config',JSON.stringify({version:2,settings:{base_url:'https://api.example/v1',model:'fixture',api_key:'DUMMY',protocol:'openai',max_steps:32,max_tokens:8192,use_max_tokens:true,language:'en',transport:'direct',stream:true,relay_token:''}})));
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#api-key')).toHaveValue('DUMMY');
});
test('saved profile credentials cannot resurrect after clear all; modal supports Escape focus return',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('https://api.example/v1');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('DUMMY_PERSISTED');
    page.once('dialog',d=>d.accept('Connection A'));await page.locator('#save-as-profile').click();await expect(page.locator('#config-status')).toContainText('saved');
    await page.locator('[data-settings-tab=advanced]').click();await page.locator('details.danger summary').click();page.once('dialog',d=>d.accept());await page.locator('#clear-config').click();
    await page.keyboard.press('Escape');await expect(page.locator('#settings-btn')).toBeFocused();await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#api-key')).toHaveValue('');
    expect(await page.evaluate(()=>JSON.stringify(localStorage).includes('DUMMY_PERSISTED'))).toBe(false);
});
test('test request and real request use normalized endpoint and custom headers',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1/');await page.locator('#model').fill('fixture');
    await page.locator('[data-settings-tab=connection]').click();await page.locator('#connection-extra summary').click();await page.locator('#custom-headers').fill('{"X-Client":"ui-test"}');await page.locator('#stream').uncheck();await page.locator('#save-config').click();
    const requests=[];await page.route('**/mock/v1/chat/completions',async route=>{requests.push(route.request());await route.fulfill({contentType:'application/json',body:JSON.stringify({choices:[{finish_reason:'stop',message:{role:'assistant',content:'OK'}}]})});});
    await page.locator('[data-settings-tab=connection]').click();await page.locator('#test-connection').click();await expect(page.locator('#test-result')).toHaveText('Connection verified.');await page.locator('#close-settings').click();
    await page.locator('#input').fill('REAL');await page.locator('#input').press('Enter');await expect(page.locator('#output')).toContainText('OK');
    expect(requests).toHaveLength(2);for(const request of requests)expect(request.headers()['x-client']).toBe('ui-test');
});

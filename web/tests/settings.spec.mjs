import {test,expect} from './fixtures.mjs';
async function open(page){await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await page.locator('#settings-btn').click();}
async function chinese(page){await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');}
test('credential inputs belong to a form without implicit submission, navigation or model requests',async({page,context,browserName})=>{
    const warnings=[],requests=[];
    if(browserName==='chromium'){
        const logs=await context.newCDPSession(page);await logs.send('Log.enable');logs.on('Log.entryAdded',({entry})=>warnings.push(entry.text));
    }
    page.on('request',request=>{if(request.method()==='POST')requests.push(request.url());});
    await open(page);
    expect(await page.evaluate(()=>{
        const key=document.getElementById('api-key'),relay=document.getElementById('relay-token');
        return key.form?.id==='connection-form'&&relay.form?.id==='relay-form'&&key.form!==relay.form;
    })).toBe(true);
    const url=page.url();await page.locator('#base-url').fill('https://api.example/v1');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('DUMMY_FORM_KEY');await page.locator('#remember-config').uncheck();
    await page.locator('#api-key').press('Enter');await expect(page.locator('#settings-modal')).toBeVisible();
    await page.locator('#transport').selectOption('relay');await page.locator('#relay-token').fill('DUMMY_RELAY_TOKEN');await page.locator('#relay-token').press('Enter');
    expect(await page.evaluate(()=>{
        const form=document.getElementById('connection-form');let cancelled=false;
        form.addEventListener('submit',event=>{cancelled=event.defaultPrevented;},{once:true});form.requestSubmit();return cancelled;
    })).toBe(true);
    await expect(page.locator('#settings-modal')).toBeVisible();expect(page.url()).toBe(url);expect(requests).toHaveLength(0);
    expect(await page.evaluate(()=>JSON.stringify(localStorage).includes('DUMMY_FORM_KEY'))).toBe(false);
    await page.locator('#transport').selectOption('direct');await page.locator('#remember-config').check();await page.locator('#save-config').click();
    expect(await page.evaluate(()=>JSON.stringify(localStorage).includes('DUMMY_FORM_KEY'))).toBe(true);expect(requests).toHaveLength(0);
    expect(warnings.filter(text=>/Password field is not contained in a form|Multiple forms should/iu.test(text))).toEqual([]);
});
test('connection options stay together, direct-only help toggles and saved values survive reload',async({page},testInfo)=>{
    await page.setViewportSize({width:390,height:844});await open(page);await chinese(page);await page.locator('[data-settings-tab=connection]').click();
    const panel=page.locator('[data-settings-panel=connection]');
    for(const id of ['transport','stream','custom-headers','cors-help'])await expect(panel.locator('#'+id)).toHaveCount(1);
    await expect(panel.locator('#transport')).toBeVisible();await expect(panel.locator('#stream')).toBeVisible();
    await expect(page.locator('#connection-extra')).not.toHaveAttribute('open','');await expect(page.locator('#cors-help')).not.toHaveAttribute('open','');
    await expect(page.locator('#connection-extra summary')).toHaveText('更多连接选项');
    await page.locator('#connection-extra summary').click();await page.locator('#custom-headers').fill('{"X-Client":"layout-fixture"}');
    await page.locator('#stream').uncheck();await page.locator('#transport').selectOption('relay');
    await expect(page.locator('#relay-token')).toBeVisible();await expect(page.locator('#connection-extra')).toBeHidden();await expect(page.locator('#cors-help')).toBeHidden();
    await page.locator('#relay-token').fill('FAKE_RELAY_ONLY');await page.locator('#transport').selectOption('direct');
    await expect(page.locator('#relay-options')).toBeHidden();await expect(page.locator('#cors-help')).toBeVisible();
    await expect(page.locator('#custom-headers')).toHaveValue('{"X-Client":"layout-fixture"}');
    await page.locator('#base-url').fill('https://api.example/v1');await page.locator('#model').fill('fixture');await page.locator('#remember-config').check();await page.locator('#save-config').click();
    for(const id of ['transport','custom-headers','stream']){const box=await page.locator('#'+id).boundingBox();expect(box.x).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(390);}
    await page.screenshot({path:testInfo.outputPath('connection-mobile.png')});
    await page.locator('[data-settings-tab=recovery]').click();await expect(page.locator('#unlimited-rounds-note')).toBeVisible();
    await page.locator('[data-settings-tab=advanced]').click();await expect(page.locator('details.danger')).toBeVisible();await expect(page.locator('[data-settings-panel=advanced] #transport')).toHaveCount(0);
    await page.reload();await expect(page.locator('#status')).toHaveText('就绪');await open(page);await page.locator('[data-settings-tab=connection]').click();
    await expect(page.locator('#transport')).toHaveValue('direct');await expect(page.locator('#stream')).not.toBeChecked();await expect(page.locator('#relay-token')).toHaveValue('FAKE_RELAY_ONLY');
    await page.locator('#connection-extra summary').click();await expect(page.locator('#custom-headers')).toHaveValue('{"X-Client":"layout-fixture"}');
    await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('en');await page.locator('[data-settings-tab=connection]').click();await expect(page.locator('#connection-extra summary')).toHaveText('More connection options');
});
test('generation settings follow the API key and native selects have themed arrows',async({page},testInfo)=>{
    await open(page);await page.locator('#base-url').fill('https://api.deepseek.com');await page.locator('#model').fill('deepseek-flash');
    const panel=page.locator('[data-settings-panel=connection]');
    await expect(panel.locator('#max-tokens')).toBeVisible();await expect(panel.locator('#reasoning-effort')).toBeVisible();
    expect(await page.evaluate(()=>{
        const key=document.getElementById('api-key'),group=document.getElementById('model-generation'),remember=document.getElementById('remember-config');
        return !!(key.compareDocumentPosition(group)&Node.DOCUMENT_POSITION_FOLLOWING)&&!!(group.compareDocumentPosition(remember)&Node.DOCUMENT_POSITION_FOLLOWING);
    })).toBe(true);
    for(const id of ['profile-select','protocol','reasoning-effort']){
        const style=await page.locator('#'+id).evaluate(el=>{const s=getComputedStyle(el);return {appearance:s.appearance,padding:parseFloat(s.paddingRight),image:s.backgroundImage};});
        expect(style.appearance).toBe('none');expect(style.padding).toBeGreaterThanOrEqual(32);expect(style.image).not.toBe('none');
    }
    await page.locator('#profile-select').focus();await page.screenshot({path:testInfo.outputPath('select-styled.png')});
    await page.setViewportSize({width:390,height:844});
    for(const id of ['profile-select','reasoning-effort','max-tokens','use-model-max']){
        const box=await page.locator('#'+id).boundingBox();expect(box.x).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(390);
    }
});
test('a delayed settings close event does not steal focus from the task input',async({page})=>{
    await open(page);await page.locator('#close-settings').click();await expect(page.locator('#settings-modal')).not.toBeVisible();
    await page.locator('#input').focus();
    await page.evaluate(()=>document.getElementById('settings-modal').dispatchEvent(new Event('close')));
    await expect(page.locator('#input')).toBeFocused();
});
test('connection selector and action buttons share an aligned desktop row',async({page})=>{
    await open(page);const boxes=await Promise.all(['profile-select','save-as-profile','delete-profile'].map(id=>page.locator('#'+id).boundingBox()));
    const bottoms=boxes.map(box=>box.y+box.height);expect(Math.max(...bottoms)-Math.min(...bottoms)).toBeLessThanOrEqual(2);
    for(const id of ['base-url','model','api-key']){const box=await page.locator('#'+id).boundingBox();expect(Math.abs(box.x-boxes[0].x)).toBeLessThanOrEqual(1);}
});
test('Chinese settings translate labels, hints, actions and all groups without replacing inputs',async({page})=>{
    await open(page);await page.locator('#api-key').fill('DUMMY_MEMORY_ONLY');await page.evaluate(()=>window.originalApiKeyInput=document.getElementById('api-key'));await chinese(page);await page.locator('[data-settings-tab=connection]').click();
    await expect(page.getByLabel('API URL',{exact:true})).toBeVisible();await expect(page.getByLabel('模型',{exact:true})).toBeVisible();await expect(page.getByLabel('API 密钥',{exact:true})).toHaveValue('DUMMY_MEMORY_ONLY');await expect(page.locator('#profile-select option').first()).toHaveText('仅当前页面');
    await expect(page.locator('#toggle-key')).toHaveText('显示');await page.locator('#toggle-key').click();await expect(page.locator('#toggle-key')).toHaveText('隐藏');
    expect(await page.evaluate(()=>window.originalApiKeyInput===document.getElementById('api-key'))).toBe(true);
    for(const group of ['connection','recovery','appearance','advanced']){await page.locator(`[data-settings-tab=${group}]`).click();const panel=page.locator(`[data-settings-panel=${group}]`);await expect(panel).not.toContainText(/Remember credentials|Connection method|API address|Maximum steps|Saved locally|Danger zone|Task and model|Saved keys are readable/);}
    await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('en');await page.locator('[data-settings-tab=connection]').click();await expect(page.getByLabel('API URL',{exact:true})).toBeVisible();await expect(page.locator('#api-key')).toHaveValue('DUMMY_MEMORY_ONLY');
});
test('language preference applies without API configuration and persists without storing a key',async({page})=>{
    await open(page);await page.locator('#api-key').fill('DUMMY_NOT_SAVED');await chinese(page);await expect(page.locator('#settings-title')).toHaveText('设置');await page.reload();await expect(page.locator('#input')).toBeEnabled();await expect(page.locator('#settings-btn')).toHaveText('设置');await expect(page.locator('#api-key')).toHaveValue('');expect(await page.evaluate(()=>JSON.stringify(localStorage).includes('DUMMY_NOT_SAVED'))).toBe(false);
});
test('Chinese settings controls fit narrow screens',async({page})=>{
    await page.setViewportSize({width:390,height:844});await open(page);await chinese(page);await page.locator('[data-settings-tab=connection]').click();
    for(const id of ['profile-select','save-as-profile','delete-profile','base-url','api-key','save-config']){const box=await page.locator('#'+id).boundingBox();expect(box.x).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(390);}
});
test('API URL accepts root and full endpoints and previews the selected protocol path',async({page})=>{
    await open(page);await expect(page.getByLabel('API URL',{exact:true})).toBeVisible();
    for(const value of ['https://api.deepseek.com','https://api.deepseek.com/chat/completions']){await page.locator('#base-url').fill(value);await expect(page.locator('#resolved-endpoint')).toHaveText('Request URL: https://api.deepseek.com/chat/completions');}
    await page.locator('#protocol').selectOption('anthropic');for(const value of ['https://api.anthropic.com','https://api.anthropic.com/v1','https://api.anthropic.com/v1/messages']){await page.locator('#base-url').fill(value);await expect(page.locator('#resolved-endpoint')).toHaveText('Request URL: https://api.anthropic.com/v1/messages');}
    await expect(page.locator('#token-parameter-option')).toBeHidden();await expect(page.locator('#anthropic-tokens-note')).toBeVisible();
});
test('Chinese connection test explains an invalid response without translating upstream errors',async({page})=>{
    await open(page);await chinese(page);await page.locator('[data-settings-tab=connection]').click();await page.locator('#base-url').fill('https://model.invalid');await page.locator('#model').fill('dummy');
    await page.route('https://model.invalid/chat/completions',route=>route.fulfill({json:{}}));await page.locator('#test-connection').click();await expect(page.locator('#test-result')).toHaveText('响应格式不正确，尚未验证模型连接。');
    await page.route('https://model.invalid/chat/completions',route=>route.fulfill({status:401,json:{error:{message:'Fixture authentication failed'}}}));await page.locator('#test-connection').click();await expect(page.locator('#test-result')).toContainText('Fixture authentication failed');
});

import {test,expect} from './fixtures.mjs';

test('browser tasks exceed 32 rounds, DeepSeek maximum and thinking controls survive reload',async({page})=>{
    test.setTimeout(90000);
    const requests=[];
    await page.route('https://api.deepseek.com/chat/completions',route=>{
        const body=route.request().postDataJSON();requests.push(body);
        const round=body.messages.filter(message=>message.role==='assistant').length;
        const tools=round<35;
        return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,finish_reason:tools?'tool_calls':'stop',delta:tools?{tool_calls:[{index:0,id:`list-${round}`,type:'function',function:{name:'list',arguments:'{}'}}]}:{content:'Completed after 35 tool rounds.'}}]})}\n\ndata: [DONE]\n\n`});
    });
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('https://api.deepseek.com');await page.locator('#model').fill('deepseek-flash');
    expect(await page.locator('#model-options option').evaluateAll(options=>options.map(option=>option.value))).toEqual(['deepseek-flash','deepseek-v4-pro']);
    await expect(page.locator('#model-help')).toContainText('not your account');
    await page.locator('#api-key').fill('FIXTURE_ONLY');
    await page.locator('[data-settings-tab=recovery]').click();
    await expect(page.locator('#max-steps')).toHaveCount(0);
    await expect(page.locator('#unlimited-rounds-note')).toBeVisible();
    await page.locator('[data-settings-tab=connection]').click();
    await page.locator('#use-model-max').click();
    await expect(page.locator('#max-tokens')).toHaveValue('393216');
    await expect(page.locator('#use-max-tokens')).toBeChecked();
    await page.locator('#reasoning-effort').selectOption('low');
    expect(await page.locator('#reasoning-effort option').evaluateAll(options=>options.map(option=>option.value))).toEqual(['default','none','low','high','max']);
    await page.locator('[data-settings-tab=connection]').click();await page.locator('#remember-config').check();
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('Continue for 35 tool rounds');await page.locator('#btn-send').click();
    await expect(page.locator('#output')).toContainText('Completed after 35 tool rounds.',{timeout:60000});
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#step-counter')).toHaveText('[step 36]');
    expect(requests).toHaveLength(36);
    for(const body of requests){expect(body.max_tokens).toBe(393216);expect(body.reasoning_effort).toBe('low');expect(body.thinking).toEqual({type:'enabled'});expect(body.max_steps).toBeUndefined();}
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#reasoning-effort')).toHaveValue('low');await expect(page.locator('#max-tokens')).toHaveValue('393216');
    expect(requests).toHaveLength(36);
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=connection]').click();
    await page.locator('#reasoning-effort').selectOption('none');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('Now without thinking');await page.locator('#btn-send').click();
    await expect(page.locator('#status')).toHaveText('Ready');
    await expect.poll(()=>requests.length).toBe(37);
    expect(requests.at(-1).thinking).toEqual({type:'disabled'});expect(requests.at(-1).reasoning_effort).toBe('none');
});

test('other providers do not advertise a guessed maximum or DeepSeek thinking settings',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('https://api.other.example/v1');await page.locator('#model').fill('custom');
    await page.locator('[data-settings-tab=connection]').click();
    await expect(page.locator('#use-model-max')).toBeHidden();await expect(page.locator('#reasoning-options')).toBeHidden();
    await expect(page.locator('#max-tokens')).toHaveValue('65536');
    await expect(page.locator('#model-options option')).toHaveCount(0);
    await expect(page.locator('#model')).toHaveValue('custom');
    await page.locator('#max-tokens').fill('12345');
    await page.locator('#save-config').click();
    await expect(page.locator('#config-status')).toHaveText('Applied for this session only.');
});

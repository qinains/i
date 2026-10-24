import {test,expect} from './fixtures.mjs';
async function configure(page,{stream=true,relay=false}={}){
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();
    await expect(page.locator('#protocol option[value=responses]')).toHaveCount(1);
    await page.locator('#protocol').selectOption('responses');await page.locator('#base-url').fill(relay?'https://api.example/v1':'http://127.0.0.1:4173/mock/v1');
    await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('TEST_RESPONSES_KEY');await page.locator('#remember-config').check();
    await page.locator('#reasoning-effort').selectOption('high');
    if(!stream)await page.locator('#stream').uncheck();
    if(relay){await page.locator('#transport').selectOption('relay');await page.locator('#relay-token').fill('fixture-relay-token-'.repeat(3));}
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
}
async function submit(page,text){await page.locator('#input').fill(text);await page.locator('#input').press('Enter');}
for(const stream of [true,false])test(`Responses real WASM ${stream?'SSE':'JSON'} preserves native tool approval and encrypted replay across reload`,async({page})=>{
    const bodies=[];page.on('request',request=>{if(request.url().endsWith('/mock/v1/responses'))bodies.push(request.postDataJSON());});
    await configure(page,{stream});await submit(page,'RESPONSES_MAKE');await expect(page.locator('#approval-panel')).toBeVisible();await expect(page.locator('#file-list')).toContainText('No files');await page.locator('#approve').click();
    await expect(page.locator('#output')).toContainText('Responses done.');await expect(page.locator('#file-list')).toContainText('responses.txt');
    expect(bodies).toHaveLength(2);expect(bodies[0].store).toBe(false);expect(bodies[0].messages).toBeUndefined();expect(bodies[0].max_output_tokens).toBe(65536);expect(bodies[0].reasoning.effort).toBe('high');
    expect(bodies[0].tools.every(tool=>tool.type==='function'&&tool.parameters&&tool.strict===false&&!tool.function&&tool.name!=='shell')).toBe(true);
    expect(bodies[1].input.some(item=>item.type==='reasoning'&&item.encrypted_content==='fixture-encrypted')).toBe(true);expect(bodies[1].input.at(-1).type).toBe('function_call_output');expect(bodies[1].input.at(-1).call_id).toBe('responses_write');
    await expect(page.locator('#output')).not.toContainText('fixture-encrypted');
    if(stream){const reasoning=page.locator('details.reasoning').first();await reasoning.locator('summary').click();await expect(reasoning.locator('pre')).toContainText('Plan files.');}
    bodies.length=0;await page.reload();await expect(page.locator('#status')).toHaveText('Ready');expect(bodies).toHaveLength(0);
    await submit(page,'continue');await expect(page.locator('#status')).toHaveText('Ready');await expect.poll(()=>bodies.length).toBe(1);expect(bodies[0].input.some(item=>item.encrypted_content==='fixture-encrypted')).toBe(true);
    await page.locator('#settings-btn').click();await expect(page.locator('#protocol')).toHaveValue('responses');await page.locator('#test-connection').click();await expect(page.locator('#test-result')).toHaveText('Connection verified.');
    expect(bodies.at(-1).max_output_tokens).toBeGreaterThan(0);expect(bodies.at(-1).store).toBe(false);expect(bodies.at(-1).input[0].role).toBe('user');
});
test('Responses truncated, incomplete and unsupported batches never execute pending tools',async({page})=>{
    await configure(page);
    for(const [task,error]of [['RESPONSES_TRUNCATE','Incomplete Responses stream'],['RESPONSES_LENGTH','Incomplete Responses response'],['RESPONSES_BAD_BATCH','unsupported Responses tool']]){
        await submit(page,task);await expect(page.locator('#output')).toContainText(error);await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#approval-panel')).not.toBeVisible();await expect(page.locator('#file-list')).toContainText('No files');
    }
});
test('Responses relay preserves native input and output with local tool approval',async({page})=>{
    await configure(page,{relay:true});await submit(page,'RESPONSES_MAKE');await expect(page.locator('#approval-panel')).toBeVisible();await page.locator('#approve').click();await expect(page.locator('#output')).toContainText('Responses done.');await expect(page.locator('#file-list')).toContainText('responses.txt');
});
test('Responses settings are localized and compact on mobile without Chat token switches',async({page})=>{
    await page.setViewportSize({width:390,height:844});await configure(page);await page.locator('#settings-btn').click();
    await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');await page.locator('[data-settings-tab=connection]').click();
    await expect(page.locator('#reasoning-label')).toHaveText('思考强度（Responses）');await expect(page.locator('#responses-note')).toContainText('max_output_tokens');await expect(page.locator('#endpoint-help')).toContainText('/v1/responses');await expect(page.locator('#token-parameter-option')).toBeHidden();
    expect(await page.locator('#settings').evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
    await page.locator('#reasoning-effort').selectOption('xhigh');await page.locator('#save-config').click();await page.locator('#close-settings').click();await page.reload();await expect(page.locator('#status')).toHaveText('就绪');await expect(page.locator('#reasoning-effort')).toHaveValue('xhigh');
});

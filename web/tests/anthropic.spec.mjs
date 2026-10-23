import {test,expect} from './fixtures.mjs';

async function configure(page,{stream=true,remember=false,relay=false}={}) {
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();
    await page.locator('#protocol').selectOption('anthropic');
    await page.locator('#base-url').fill(relay?'https://api.example/v1':'http://127.0.0.1:4173/mock/v1');
    await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('TEST_ANTHROPIC_KEY');
    if(remember)await page.locator('#remember-config').check();
    if(!stream || relay) {
        await page.locator('[data-settings-tab=connection]').click();
        if(!stream)await page.locator('#stream').uncheck();
        if(relay){await page.locator('#transport').selectOption('relay');await page.locator('#relay-token').fill('fixture-relay-token-'.repeat(3));}
        await page.locator('[data-settings-tab=connection]').click();
    }
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
}
async function submit(page,text){await page.locator('#input').fill(text);await page.locator('#input').press('Enter');}

for(const stream of [true,false])test(`real WASM Anthropic ${stream?'streaming':'JSON'} tools use native messages and approval`,async({page})=>{
    const bodies=[];page.on('request',request=>{if(request.url().endsWith('/mock/v1/messages'))bodies.push(request.postDataJSON());});
    await configure(page,{stream});await submit(page,'ANTHROPIC_MAKE');
    await expect(page.locator('#approval-panel')).toBeVisible();
    await expect(page.locator('#file-list')).toContainText('No files');await page.locator('#approve').click();
    await expect(page.locator('#output')).toContainText('Anthropic done.');await expect(page.locator('#file-list')).toContainText('anthropic.txt');
    expect(bodies).toHaveLength(2);expect(bodies[0].system[0].type).toBe('text');expect(bodies[0].max_tokens).toBeGreaterThan(0);
    expect(bodies[0].tools.every(tool=>tool.input_schema && tool.name!=='shell')).toBe(true);
    const assistant=bodies[1].messages.find(message=>message.role==='assistant');
    expect(assistant.content[0]).toEqual({type:'thinking',thinking:'private reasoning',signature:'fixture-signature'});
    expect(assistant.content[1].type).toBe('tool_use');expect(bodies[1].messages.at(-1).content[0].type).toBe('tool_result');
    if(stream){
        const thinking=page.locator('details.reasoning').first();await expect(thinking.locator('pre')).toBeHidden();
        await thinking.locator('summary').click();await expect(thinking.locator('pre')).toHaveText('private reasoning');
        await expect(page.locator('#output')).not.toContainText('fixture-signature');
    }
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'anthropic.txt',100));})).toBe('native messages\n');
});

test('Anthropic truncated stream and mixed unsupported tool batch never request approval or write',async({page})=>{
    await configure(page);
    await submit(page,'ANTHROPIC_TRUNCATE');await expect(page.locator('#output')).toContainText('Incomplete Anthropic stream');
    await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#approval-panel')).not.toBeVisible();
    await submit(page,'ANTHROPIC_BAD_BATCH');await expect(page.locator('#output')).toContainText('unsupported Anthropic tool');
    await expect(page.locator('#approval-panel')).not.toBeVisible();await expect(page.locator('#file-list')).toContainText('No files');
});

test('Anthropic saved connection restores opaque history without sending on reload; connection test uses same native auth',async({page})=>{
    await configure(page,{remember:true});await submit(page,'ANTHROPIC_MAKE');
    await expect(page.locator('#approval-panel')).toBeVisible();await page.locator('#approve').click();await expect(page.locator('#output')).toContainText('Anthropic done.');
    const bodies=[];page.on('request',request=>{if(request.url().endsWith('/mock/v1/messages'))bodies.push(request.postDataJSON());});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');expect(bodies).toHaveLength(0);await expect(page.locator('#protocol')).toHaveValue('anthropic');
    await submit(page,'continue');await expect(page.locator('#status')).toHaveText('Ready');await expect.poll(()=>bodies.length).toBe(1);
    expect(bodies[0].messages.find(message=>message.role==='assistant').content[0].signature).toBe('fixture-signature');
    await page.locator('#settings-btn').click();await page.locator('#test-connection').click();await expect(page.locator('#test-result')).toHaveText('Connection verified.');
    expect(bodies).toHaveLength(2);expect(bodies[1].stream).toBe(false);expect(bodies[1].max_tokens).toBeGreaterThan(0);expect(bodies[1].max_completion_tokens).toBeUndefined();expect(bodies[1].messages[0].content[0].type).toBe('text');
});

test('Anthropic relay retains native protocol, authentication and approval',async({page})=>{
    await configure(page,{relay:true});await submit(page,'ANTHROPIC_MAKE');
    await expect(page.locator('#approval-panel')).toBeVisible();await page.locator('#approve').click();
    await expect(page.locator('#output')).toContainText('Anthropic done.');await expect(page.locator('#file-list')).toContainText('anthropic.txt');
});

import {test,expect} from './fixtures.mjs';
async function open(page){await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#plugins-btn').click();await expect(page.locator('#prompt-system')).toBeVisible();}
test('default prompt plugin has no file permission and user edits survive reload',async({page})=>{
    await open(page);await expect(page.locator('[data-plugin-id="builtin.prompts"]')).toContainText('Enabled');
    await page.locator('#prompt-system').fill('CUSTOM SYSTEM PROMPT');await page.locator('#save-prompts').click();await expect(page.locator('#prompt-status')).toContainText('Saved');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#plugins-btn').click();await expect(page.locator('#prompt-system')).toHaveValue('CUSTOM SYSTEM PROMPT');
    page.once('dialog',dialog=>dialog.accept());await page.locator('[data-plugin-id="builtin.prompts"]').getByRole('button',{name:'Clear private data…'}).click();await expect(page.locator('#plugin-status')).toHaveText('Private plugin data cleared.');
    await page.locator('[data-plugin-id="builtin.prompts"] [data-plugin-action=disable]').click();
    await expect(page.locator('[data-plugin-id="builtin.prompts"]')).toContainText('Disabled');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#plugins-btn').click();await expect(page.locator('[data-plugin-id="builtin.prompts"]')).toContainText('Disabled');
});
for(const allowed of [true,false])test(`AI prompt changes require confirmation even in auto mode (${allowed?'approve':'deny'})`,async({page})=>{
    await open(page);await page.locator('#close-plugins').click();await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=connection]').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('DUMMY_PROMPT_KEY');await page.locator('#save-config').click();await page.locator('#close-settings').click();page.on('dialog',dialog=>dialog.accept());await page.locator('#execution-mode').selectOption('auto');
    const bodies=[];await page.route('**/mock/v1/chat/completions',route=>{
        const body=route.request().postDataJSON();bodies.push(body);const tool=body.tools.find(t=>t.function.name.startsWith('p_manage_prompt_'))?.function.name;
        const first=bodies.length===1,delta=first?{tool_calls:[{index:0,id:'prompt-change',type:'function',function:{name:tool,arguments:JSON.stringify({action:'set',prompt:'CONFIRMED NEW SYSTEM'})}}]}:{content:'Done'};
        return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:first?'tool_calls':'stop'}]})}\n\ndata: [DONE]\n\n`});
    });
    await page.locator('#input').fill('Change my prompt');await page.locator('#input').press('Enter');await expect(page.locator('#approval-panel')).toBeVisible();await expect(page.locator('#approval-description')).toContainText('CONFIRMED NEW SYSTEM');await page.locator(allowed?'#approve':'#deny').click();await expect(page.locator('#status')).toHaveText('Ready');
    expect(bodies[0].messages[0].content).not.toBe('CONFIRMED NEW SYSTEM');expect(bodies[1].messages[0].content).not.toBe('CONFIRMED NEW SYSTEM');
    await page.locator('#input').fill('Continue');await page.locator('#input').press('Enter');await expect(page.locator('#status')).toHaveText('Ready');await expect.poll(()=>bodies.length).toBe(3);expect(bodies[2].messages[0].content).toBe(allowed?'CONFIRMED NEW SYSTEM':bodies[0].messages[0].content);
    expect(JSON.stringify(bodies[0].tools)).not.toContain('DUMMY_PROMPT_KEY');
});

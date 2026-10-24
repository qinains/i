import {test,expect} from './fixtures.mjs';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const packages=['example.word-count','example.json-format'].map(id=>({id,path:fileURLToPath(new URL(`../../dist/plugins/${id}-1.0.0.hplugin.zip`,import.meta.url))}));
test('two real Rust WASM plugins install together, execute with scoped permissions, and survive reload',async({page},testInfo)=>{
    await page.setViewportSize({width:1440,height:1200});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=plugins]').click();
    for(const plugin of packages){
        await expect(page.locator('#plugin-file')).toBeEnabled();await page.locator('#plugin-file').setInputFiles(plugin.path);
        await expect(page.locator('#plugin-preview')).toContainText(plugin.id);await expect(page.locator('#plugin-preview')).toContainText('Unverified publisher');
        await page.locator('#plugin-install').click();const card=page.locator(`[data-plugin-id="${plugin.id}"]`);await expect(card).toContainText('Disabled');
        page.once('dialog',dialog=>dialog.accept());await card.locator('[data-plugin-action=enable]').click();await expect(card).toContainText('Enabled');
    }
    await expect(page.locator('#plugin-list [data-plugin-id]')).toHaveCount(3);await expect(page.locator('[data-plugin-id="builtin.prompts"]')).toContainText('Enabled');
    await page.locator('[data-settings-tab=connection]').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    for(const [name,content] of [['notes.txt','hello world'],['input.json','{"b":2,"a":1}']]){
        await page.locator('#file-input').setInputFiles({name,mimeType:'text/plain',buffer:Buffer.from(content)});await expect(page.locator(`[data-file="${name}"]`)).toHaveCount(1);
    }
    const names=await page.evaluate(async()=>{
        const {openPluginStore}=await import('/plugins/store.js'),{PluginManager}=await import('/plugins/manager.js'),{inspectPluginPackage}=await import('/plugins/runner.js');
        const store=await openPluginStore();try{return await new PluginManager({store,inspectPackage:inspectPluginPackage}).withSnapshot('default',snapshot=>Object.fromEntries(snapshot.tools.map(tool=>[tool.pluginId,tool.publicName])));}finally{store.close();}
    });
    expect(Object.keys(names).sort()).toEqual([...packages.map(p=>p.id),'builtin.prompts'].sort());const requests=[];
    await page.route('**/mock/v1/chat/completions',route=>{
        const body=route.request().postDataJSON();requests.push(body);const first=body.messages.at(-1).role==='user';
        const calls=first?[[names['example.word-count'],{path:'notes.txt'}],[names['example.json-format'],{path:'input.json',output_path:'formatted.json'}]].map(([name,args],index)=>({index,id:`plugin-${index}`,type:'function',function:{name,arguments:JSON.stringify(args)}})):[];
        const delta={role:'assistant',content:first?null:'Two plugins completed.',...(first?{tool_calls:calls}:{})};
        return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:first?'tool_calls':'stop'}]})}\n\ndata: [DONE]\n\n`});
    });
    await page.locator('#input').fill('Use both installed plugins: count notes and format JSON');await page.locator('#input').press('Enter');
    for(let i=0;i<3;i++){await expect(page.locator('#approval-panel')).toBeVisible();if(i===2)await expect(page.locator('#approval-description')).toContainText('formatted.json');await page.locator('#approve').click();}
    await expect(page.locator('#output')).toContainText('Two plugins completed.');await expect(page.locator('#output')).toContainText('"words":2');await expect(page.locator('#status')).toHaveText('Ready');expect(requests).toHaveLength(2);
    const readFormatted=()=>page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'formatted.json',1000));});
    expect(await readFormatted()).toBe('{\n  "a": 1,\n  "b": 2\n}\n');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');expect(await readFormatted()).toBe('{\n  "a": 1,\n  "b": 2\n}\n');expect(requests).toHaveLength(2);
    await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=plugins]').click();
    for(const plugin of packages)await expect(page.locator(`[data-plugin-id="${plugin.id}"]`)).toContainText('Enabled');
    const word=page.locator('[data-plugin-id="example.word-count"]');await word.locator('[data-plugin-action=disable]').click();await expect(word).toContainText('Disabled');await expect(page.locator('[data-plugin-id="example.json-format"]')).toContainText('Enabled');
    page.once('dialog',dialog=>dialog.accept());await word.locator('[data-plugin-action=enable]').click();await expect(word).toContainText('Enabled');
    await page.screenshot({path:testInfo.outputPath('two-plugins-installed.png'),fullPage:true});
    await page.locator('#close-settings').click();const download=page.waitForEvent('download');await page.locator('#btn-download').click();const artifact=await download;
    const zip=await readFile(await artifact.path());expect(zip.includes(Buffer.from('formatted.json'))).toBe(true);expect(zip.includes(Buffer.from('plugin.wasm'))).toBe(false);
});

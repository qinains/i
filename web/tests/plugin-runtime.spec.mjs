import {test,expect} from './fixtures.mjs';
import {readFile} from 'node:fs/promises';
import {pluginPackage} from './plugin-fixtures.mjs';
const fixture=async()=>[...await readFile(new URL('../../target/browser-plugin-fixtures/example.word-count-1.0.0.hplugin.zip',import.meta.url))];
test('plugin runtime validates without guest execution and runs real Rust SDK yield resume',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const result=await page.evaluate(async bytes=>{
        const {inspectPluginPackage,createPluginRun}=await import('/plugins/runner.js');
        const v=await inspectPluginPackage(new Uint8Array(bytes));const descriptor=v.descriptors[0];let calls=0;
        const run=createPluginRun({descriptor,bytes:v.wasm,request:{id:'count1',event:'tool',workspace_id:'default',data:{name:'count_words',arguments:{path:'notes.txt'}}},onHostCall:async request=>{calls++;return {id:request.id,ok:true,result:{text:'one two 中文'}};}});
        return {output:await run.result,calls,manifest:v.manifest.id};
    },await fixture());
    expect(result).toEqual({output:{output:'{"characters":10,"words":3}'},calls:1,manifest:'example.word-count'});
});
for(const name of ['loop','abi-loop','bad-pointer','trap','oversized-output','repeated-host'])test(`plugin runtime bounds untrusted ${name} without blocking the page`,async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.evaluate(async bytes=>{
        const {inspectPluginPackage,createPluginRun}=await import('/plugins/runner.js');const v=await inspectPluginPackage(new Uint8Array(bytes));
        const run=createPluginRun({descriptor:v.descriptors[0],bytes:v.wasm,request:{id:'bad',event:'tool',workspace_id:'default',data:{name:'count_words',arguments:{}}},timeoutMs:1500,onHostCall:async r=>({id:r.id,ok:true,result:{text:'one'}})});
        window.pluginFailure=run.result.then(()=>null,error=>String(error.message));return true;
    },await pluginPackage({},name));
    await page.locator('#settings-btn').click();await expect(page.locator('#settings-modal')).toBeVisible();
    expect(await page.evaluate(()=>window.pluginFailure)).toMatch(/timed out|memory range|unreachable|step output|sequence|operation limit/iu);
});

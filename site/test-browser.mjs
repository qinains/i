import assert from 'node:assert/strict';
import {mkdtemp,rm,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium,webkit} from '../web/node_modules/playwright/index.mjs';

const profile=await mkdtemp(join(tmpdir(),'lninl-site-'));
const origin=process.env.SITE_TEST_ORIGIN||'http://localhost:8090';
await mkdir(new URL('../dist/site-screenshots/',import.meta.url),{recursive:true});
try{
    for(const [name,browser]of Object.entries({chromium,webkit})){
        const context=await browser.launchPersistentContext(join(profile,name),{headless:true,viewport:{width:1440,height:1000}}),page=await context.newPage(),errors=[];
        page.on('pageerror',error=>errors.push(error.message));
        try{
            await page.goto(origin+'/');await page.getByRole('heading',{name:/你的工具/}).waitFor();
            await page.screenshot({path:new URL(`../dist/site-screenshots/${name}-home.png`,import.meta.url).pathname,fullPage:true});
            await page.locator('#search').fill('harness-wasm-v1');await page.locator('#search-results a').first().waitFor();
            await page.goto(origin+'/docs/plugin-development/');assert.ok(await page.locator('article').textContent().then(text=>text.includes('hplugin.zip')));
            assert.equal(await page.locator('a[rel=canonical]').count(),0);
            await page.setViewportSize({width:390,height:844});await page.reload();assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
            await page.locator('.docs-menu summary').first().click();await page.getByRole('navigation',{name:'文档',exact:true}).waitFor();
            await page.screenshot({path:new URL(`../dist/site-screenshots/${name}-docs-mobile.png`,import.meta.url).pathname,fullPage:false});
            await page.goto(origin+'/app/');await page.locator('#input:not(:disabled)').waitFor({timeout:20000});
            await page.locator('#settings-btn').click();await page.locator('#cors-help summary').click();assert.equal(await page.locator('#cors-help a[download]').count(),3);
            assert.deepEqual(errors,[]);console.log(`PASS ${name}: homepage, search, plugin docs, mobile navigation/overflow, /app/ WASM and extension links`);
        }finally{await context.close();}
    }
}finally{await rm(profile,{recursive:true,force:true});}

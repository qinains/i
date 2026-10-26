import assert from 'node:assert/strict';
import {chromium,webkit} from '../web/node_modules/playwright/index.mjs';
import {catalogs} from './assets/i18n.js';
import {languageOptions} from './assets/languages.js';
const origin=process.env.SITE_TEST_ORIGIN||'http://localhost:8090';
const url='https://qinains.github.io/i-plugins/index.json',hash='a'.repeat(64);
const entry={id:'example.word-count',name:'Word count',version:'1.0.0',description:'<img src=x onerror="window.pluginXSS=1">',repository:'https://github.com/qinains/i',source_revision:'b'.repeat(40),license:'MIT',package:{url:`https://qinains.github.io/i-plugins/packages/${hash}.hplugin.zip`,sha256:hash,size:100},permissions:{workspace:'read',storage:false}};
for(const [name,type]of Object.entries({chromium,webkit})){
    const browser=await type.launch({headless:true});
    try{
        const page=await browser.newPage(),errors=[];let failure=false;
        page.on('pageerror',error=>errors.push(error.message));
        await page.route(url,route=>route.fulfill({status:failure?503:200,contentType:'application/json',headers:{'access-control-allow-origin':'*'},body:JSON.stringify({schema_version:1,plugins:[entry]})}));
        await page.goto(origin+'/plugins/');await page.locator('#plugins-list article').waitFor();
        assert.ok((await page.locator('#plugins-list').textContent()).includes(entry.description));assert.equal(await page.locator('#plugins-list img').count(),0);assert.equal(await page.evaluate(()=>window.pluginXSS),undefined);
        assert.equal(await page.locator('#plugins-list a').first().getAttribute('href'),entry.repository);
        assert.equal(await page.locator('.plugin-actions a.button').getAttribute('href'),'/app/?settings=plugins');
        for(const {code}of languageOptions){
            await page.locator('#site-language').selectOption(code);assert.equal(await page.locator('h1').textContent(),catalogs[code]['plugins.title']);
        }
        await page.setViewportSize({width:320,height:844});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
        failure=true;await page.locator('#plugins-load').click();await page.waitForFunction(()=>!document.querySelector('#plugins-load').disabled);
        assert.equal(await page.locator('#plugins-list article').count(),0);assert.equal(await page.locator('#plugins-status').textContent(),catalogs.th['plugins.error']);assert.deepEqual(errors,[]);
        console.log(`PASS ${name}: localized plugin directory, escaped metadata, safe links, mobile layout and network failure`);
    }finally{await browser.close();}
}

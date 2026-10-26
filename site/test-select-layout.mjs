import assert from 'node:assert/strict';
import {chromium,webkit} from '../web/node_modules/playwright/index.mjs';

const origin=process.env.SITE_TEST_ORIGIN||'http://localhost:8090';
for(const [name,type]of Object.entries({chromium,webkit})){
    const browser=await type.launch({headless:true});
    try{
        const page=await browser.newPage();await page.goto(origin+'/');
        await page.locator('#site-language option[value=ar]').waitFor({state:'attached'});
        for(const width of [1440,390,320])for(const language of ['zh-CN','ar']){
            await page.setViewportSize({width,height:844});await page.locator('#site-language').selectOption(language);
            const style=await page.locator('#site-language').evaluate(element=>{const s=getComputedStyle(element);return {appearance:s.appearance,image:s.backgroundImage,position:s.backgroundPosition,left:parseFloat(s.paddingLeft),right:parseFloat(s.paddingRight),width:element.clientWidth};});
            assert.equal(style.appearance,'none');assert.notEqual(style.image,'none');
            assert.equal(style.position,language==='ar'?'13px 50%, 18px 50%':'calc(100% - 18px) 50%, calc(100% - 13px) 50%');
            assert.ok((language==='ar'?style.left:style.right)>=32);
            assert.ok(style.width>=90,`${name}: language label must retain room beside the arrow`);
            assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
        }
        await page.emulateMedia({forcedColors:'active'});
        if(await page.evaluate(()=>matchMedia('(forced-colors:active)').matches)){
            const native=await page.locator('#site-language').evaluate(element=>{const s=getComputedStyle(element);return [s.appearance,s.backgroundImage];});
            assert.deepEqual(native,['auto','none']);
        }
        console.log(`PASS ${name}: language selector arrow gutters, RTL, mobile widths and forced-color native fallback`);
    }finally{await browser.close();}
}

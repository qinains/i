import assert from 'node:assert/strict';
import {chromium,webkit} from '../web/node_modules/playwright/index.mjs';

const origin=process.env.SITE_TEST_ORIGIN||'http://localhost:8090';
function luminance(rgb){
    const channels=rgb.slice(0,3).map(value=>{const v=value/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;});
    return channels[0]*.2126+channels[1]*.7152+channels[2]*.0722;
}
for(const [name,type]of Object.entries({chromium,webkit})){
    const browser=await type.launch({headless:true});
    try{
        const page=await browser.newPage({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
        for(const path of ['/','/docs/plugin-development/']){
            await page.goto(origin+path);await page.locator('#site-language option[value=ar]').waitFor({state:'attached'});
            for(const scheme of ['light','dark'])for(const theme of ['auto','light','dark']){
                await page.emulateMedia({colorScheme:scheme});
                await page.evaluate(theme=>{if(theme==='auto')delete document.documentElement.dataset.theme;else document.documentElement.dataset.theme=theme;},theme);
                for(const button of await page.locator('a.button:visible').all())for(const state of ['normal','hover','focus']){
                    await page.mouse.move(0,0);
                    if(state==='hover')await button.hover();else if(state==='focus')await button.focus();
                    const colors=await button.evaluate(element=>{const style=getComputedStyle(element);return {text:style.color,background:style.backgroundColor,label:element.textContent};});
                    const text=colors.text.match(/[\d.]+/g).map(Number),background=colors.background.match(/[\d.]+/g).map(Number);
                    if(background[3]===0)continue;
                    const values=[luminance(text),luminance(background)].sort((a,b)=>b-a),ratio=(values[0]+.05)/(values[1]+.05);
                    assert.ok(ratio>=4.5,`${name} ${path} system=${scheme} theme=${theme} ${state} ${colors.label}: contrast ${ratio.toFixed(2)} (${colors.text} / ${colors.background})`);
                }
            }
        }
        console.log(`PASS ${name}: filled button text contrast >= 4.5:1 across home/docs, system/explicit themes, hover and focus`);
    }finally{await browser.close();}
}

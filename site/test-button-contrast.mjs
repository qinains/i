import assert from 'node:assert/strict';
import {chromium,webkit} from '../web/node_modules/playwright/index.mjs';
import {controlMetrics} from './control-contrast.mjs';

const origin=process.env.SITE_TEST_ORIGIN||'http://localhost:8090';
const failures=new Set();
for(const [name,type]of Object.entries({chromium,webkit})){
    const browser=await type.launch({headless:true});
    try{
        const page=await browser.newPage({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
        for(const path of ['/','/docs/plugin-development/']){
            await page.goto(origin+path);await page.locator('#site-language option[value=ar]').waitFor({state:'attached'});
            for(const width of [1440,390])for(const scheme of ['light','dark'])for(const theme of ['auto','light','dark']){
                await page.setViewportSize({width,height:1000});
                await page.emulateMedia({colorScheme:scheme});
                await page.evaluate(theme=>{if(theme==='auto')delete document.documentElement.dataset.theme;else document.documentElement.dataset.theme=theme;},theme);
                for(const button of await page.locator('a.button:visible,button:visible,select:visible,summary:visible').all())for(const state of ['normal','hover','focus']){
                    await page.mouse.move(0,0);
                    if(state==='hover')await button.hover();else if(state==='focus')await button.focus();
                    const metrics=await button.evaluate(controlMetrics);if(metrics.disabled)continue;
                    if(metrics.ratio<(metrics.iconOnly?3:4.5))failures.add(`${name} ${path} system=${scheme} theme=${theme} ${state} ${metrics.label}: contrast ${metrics.ratio.toFixed(2)}`);
                }
            }
        }
        console.log(`Checked ${name}: home/docs controls, including transparent controls, across themes, hover and focus`);
    }finally{await browser.close();}
}
assert.deepEqual([...failures],[]);

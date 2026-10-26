import {test,expect} from './fixtures.mjs';
import {controlMetrics} from '../../site/control-contrast.mjs';

test('workspace control colors remain readable throughout settings, menus and themes',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#plugins-btn').click();
    // Reveal existing controls without invoking destructive actions or any model requests.
    await page.evaluate(()=>{
        document.querySelector('#plugins-modal').close();
        for(const element of document.querySelectorAll('[hidden]')){
            if(element.closest('#settings-modal')&&!element.matches('[data-settings-panel]'))element.hidden=false;
        }
    });
    let checked=0;const failures=new Set();
    for(const width of [1280,390])for(const theme of ['light','dark']){
        await page.setViewportSize({width,height:844});
        await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
        const audit=async()=>{
            for(const metrics of await page.locator('button:visible,select:visible,summary:visible').evaluateAll(controlMetrics)){
                if(metrics.disabled||metrics.opacity<1)continue;
                if(metrics.ratio<(metrics.iconOnly?3:4.5))failures.add(`${theme}/${width} ${metrics.label}: contrast ${metrics.ratio.toFixed(2)}`);
                if(metrics.height<24)failures.add(`${width} ${metrics.label}: target height ${metrics.height.toFixed(1)}`);checked++;
            }
        };
        await audit();await page.locator('#settings-btn').click();
        for(const tab of ['connection','recovery','appearance','advanced']){
            await page.locator(`[data-settings-tab=${tab}]`).click();await audit();
        }
        await page.locator('#close-settings').click();await expect(page.locator('#settings-modal')).not.toBeVisible();
        await page.locator('#plugins-btn').click();await audit();await page.locator('#close-plugins').click();await expect(page.locator('#plugins-modal')).not.toBeVisible();
        await page.locator('#input').fill('/');await audit();await page.locator('#input').press('Escape');await page.locator('#input').fill('');
        const filename=`controls-${theme}-${width}.html`;
        await page.locator('#file-input').setInputFiles({name:filename,mimeType:'text/html',buffer:Buffer.from('<h1>Control audit fixture</h1>')});
        if(!await page.locator('#file-panel').isVisible())await page.locator('#btn-files').click();
        await page.locator(`button[data-file="${filename}"]`).click();
        if(width<=700)await page.locator('#preview-options>summary').click();
        await audit();await page.locator('#preview-fullscreen').click();await audit();
        await page.locator('#preview-exit-fullscreen').click();await page.locator('#close-preview').click();
    }
    expect(checked).toBeGreaterThan(100);
    expect([...failures]).toEqual([]);
});

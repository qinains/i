import {test as base,expect} from './app-fixtures.mjs';
import {setTimeout as delay} from 'node:timers/promises';

const test=base.extend({
    page:async({page},use)=>{
        await page.route('**/pkg/i_bg.wasm',async route=>{await delay(6000);await route.continue();});
        await use(page);
    },
});
test.setTimeout(5000);
test('cold WASM initialization has a separate budget from application behavior',async({readyPage:page},testInfo)=>{
    await expect(page.locator('#status')).toHaveText('Ready');
    expect(testInfo.timeout).toBe(5000);
});

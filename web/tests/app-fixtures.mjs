import {test as base,expect} from './fixtures.mjs';

export const test=base.extend({
    readyPage:[async({page},use)=>{
        await page.goto('/');
        await expect(page.locator('#status')).toHaveText('Ready',{timeout:20_000});
        await use(page);
    },{timeout:30_000}],
});
export {expect};

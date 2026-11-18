import {test,expect} from './fixtures.mjs';
test('queue save failure leaves messages visible in memory and pauses dispatch',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.evaluate(()=>{
        const original=window.fetch;window.queueHits=0;
        window.fetch=(url,options)=>{
            if(!String(url).includes('/mock/v1/chat/completions'))return original(url,options);
            window.queueHits++;return new Promise((resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new DOMException('Aborted','AbortError')),{once:true}));
        };
    });
    await page.locator('#input').fill('first');await page.locator('#btn-send').click();await expect(page.locator('#input')).toBeEnabled();
    await expect.poll(()=>page.evaluate(()=>window.queueHits)).toBe(1);
    await page.evaluate(async()=>{
        const {SessionController}=await import('/sessions.js'),original=SessionController.prototype.save;
        SessionController.prototype.save=function(record){
            if(record.pendingMessages?.length||this.failed.has(record.id)){
                const error=new Error('Fixture queue save failed');this.failed.set(record.id,error);this.hooks.saving?.(record.id,'Save failed');return Promise.reject(error);
            }
            return original.call(this,record);
        };
    });
    await page.locator('#input').fill('Keep this unsaved message');await page.locator('#btn-send').click();
    await expect(page.locator('#global-error')).toContainText('Fixture queue save failed');
    await expect(page.locator('.pending-message')).toContainText('Keep this unsaved message');
    await expect(page.locator('#queue-resume')).toBeVisible();await expect(page.locator('#save-status')).toHaveText('Save failed');
    expect(await page.evaluate(()=>window.queueHits)).toBe(1);
    await page.locator('.pending-message').getByRole('button',{name:'Cancel message',exact:true}).click();
    await expect(page.locator('#pending-messages')).toBeHidden();
});

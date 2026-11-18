import {test,expect} from './fixtures.mjs';

test('composer action labels switch with every supported language on mobile',async({page})=>{
    test.setTimeout(90000);await page.setViewportSize({width:390,height:844});
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.evaluate(()=>{const original=window.fetch;window.fetch=(url,options)=>String(url).includes('/mock/v1/chat/completions')?new Promise((resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new DOMException('Aborted','AbortError')),{once:true})):original(url,options);});
    for(const [language,labels]of Object.entries({
        en:['Send','Queue message','Stop'],'zh-CN':['发送','追加','停止'],
        'zh-TW':['傳送','追加','停止'],ja:['送信','追加','停止'],
        ko:['보내기','추가','중지'],fr:['Envoyer','Ajouter à la file','Arrêter'],de:['Senden','Einreihen','Stoppen'],
    })){
        await page.locator('#settings-btn').click();await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption(language);await page.locator('#close-settings').click();
        await expect(page.locator('#btn-send')).toHaveText(labels[0]);await page.locator('#input').fill('first');await page.locator('#btn-send').click();await expect(page.locator('#input')).toBeEnabled();
        await expect(page.locator('#btn-cancel')).toBeVisible();await expect(page.locator('#btn-cancel')).toHaveText(labels[2]);await expect(page.locator('#btn-send')).toBeHidden();
        await page.locator('#input').fill('follow-up');await expect(page.locator('#btn-send')).toBeVisible();await expect(page.locator('#btn-send')).toHaveText(labels[1]);await expect(page.locator('#btn-cancel')).toBeHidden();
        expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
        await page.locator('#input').fill('');await page.locator('#btn-cancel').click();await expect(page.locator('#btn-cancel')).toBeHidden();await expect(page.locator('#btn-send')).toHaveText(labels[0]);await expect(page.locator('#settings-btn')).toBeEnabled();
    }
});

import {test,expect} from './fixtures.mjs';
import {copy} from '../ui-copy.js';
import {t} from '../i18n.js';
import {languageOptions} from '../languages.js';

test('all seventeen UI languages translate settings without sending requests or changing credentials',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const requests=[];page.on('request',request=>{if(request.method()==='POST')requests.push(request.url());});
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('https://fixture.invalid/v1');await page.locator('#model').fill('fixture-model');await page.locator('#api-key').fill('FIXTURE_LANGUAGE_TEST_ONLY');await page.locator('#remember-config').check();await page.locator('#save-config').click();
    await page.locator('[data-settings-tab=appearance]').click();await expect(page.locator('#lang-select option')).toHaveCount(17);
    await page.evaluate(async()=>{const {renderMessage}=await import('./render.js');const output=document.querySelector('#output');output.append(renderMessage({type:'reasoning',pending:true,text:'Original model text'}),renderMessage({type:'assistant',text:'Settings'}));});
    for(const {code,dir}of languageOptions){
        await page.locator('#lang-select').selectOption(code);await expect(page.locator('html')).toHaveAttribute('lang',code);await expect(page.locator('html')).toHaveAttribute('dir',dir);
        await expect(page.locator('#save-config')).toHaveText(copy[code]['Save changes']);await expect(page.locator('[data-settings-tab=connection]')).toHaveText(copy[code]['Model connection']);
        await expect(page.locator('#workspace-select option[value=__new__]')).toContainText(copy[code]['New workspace']);await expect(page.locator('#workspace-select option[value=__scratch__]')).toContainText(copy[code].Scratch);
        await expect(page.locator('.line.reasoning summary')).toHaveText(copy[code]['Thinking…']);await expect(page.locator('.line.assistant')).toContainText('Settings');
        await expect(page.locator('[role=tab][aria-selected=true]')).toHaveText(copy[code]['New conversation']);await expect(page.locator('.tab-close').first()).toHaveAttribute('aria-label',t('Close {name}',code,{name:copy[code]['New conversation']}));
        await expect(page.locator('#base-url')).toHaveValue('https://fixture.invalid/v1');await expect(page.locator('#model')).toHaveValue('fixture-model');await expect(page.locator('#api-key')).toHaveValue('FIXTURE_LANGUAGE_TEST_ONLY');
    }
    await page.locator('#lang-select').selectOption('es');await page.locator('[data-settings-tab=connection]').click();
    const savedConnection=await page.locator('#profile-select').inputValue();expect(savedConnection).not.toBe('');
    await page.locator('#profile-select').selectOption('');await page.locator('#profile-select').selectOption(savedConnection);await expect(page.locator('html')).toHaveAttribute('lang','es');
    await page.reload();await expect(page.locator('html')).toHaveAttribute('lang','es');await expect(page.locator('#status')).toHaveText(copy.es.Ready);
    await page.locator('#settings-btn').click();await expect(page.locator('#api-key')).toHaveValue('FIXTURE_LANGUAGE_TEST_ONLY');expect(requests).toEqual([]);
});

test.describe('Arabic system locale',()=>{
    test.use({locale:'ar-SA'});
    test('first visit follows Arabic, mirrors UI, but preserves LTR commands, code and URLs',async({page})=>{
        await page.goto('/');await expect(page.locator('html')).toHaveAttribute('lang','ar');await expect(page.locator('html')).toHaveAttribute('dir','rtl');await expect(page.locator('#status')).toHaveText(copy.ar.Ready);
        await page.locator('#settings-btn').click();await page.locator('#base-url').fill('https://fixture.invalid/v1');expect(await page.locator('#base-url').evaluate(element=>getComputedStyle(element).direction)).toBe('ltr');
        await page.locator('[data-settings-tab=appearance]').click();await expect(page.locator('#lang-select')).toHaveValue('ar');await page.setViewportSize({width:320,height:780});expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
        await page.locator('#close-settings').click();await page.locator('#input').fill('/');await expect(page.locator('#command-menu')).toBeVisible();
        await expect(page.locator('#command-menu')).toContainText(copy.ar['Open workspace files']);expect(await page.locator('#command-menu strong').first().evaluate(element=>getComputedStyle(element).direction)).toBe('ltr');expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
        await page.locator('#input').press('Escape');await page.locator('#input').fill('مرحبا بالعالم');expect(await page.locator('#input').evaluate(element=>getComputedStyle(element).direction)).toBe('rtl');
        await page.locator('#input').fill('/mode');expect(await page.locator('#input').evaluate(element=>getComputedStyle(element).direction)).toBe('ltr');
        await page.setViewportSize({width:1280,height:844});if(!await page.locator('#file-panel').isVisible())await page.locator('#btn-files').click();
        const files=await page.locator('#file-panel').boundingBox(),terminal=await page.locator('#terminal').boundingBox();expect(files.x).toBeGreaterThanOrEqual(terminal.x+terminal.width-1);
        const content=await page.evaluate(async()=>{const {renderMessage}=await import('./render.js');const record={type:'assistant',text:'const value = 1;'};const element=renderMessage(record);document.querySelector('#output').append(element);return {text:element.firstChild.textContent,dir:element.dir};});
        expect(content).toEqual({text:'const value = 1;',dir:'auto'});
        await page.screenshot({path:new URL('../../dist/site-screenshots/workspace-arabic.png',import.meta.url).pathname,animations:'disabled'});
    });
    test('queued English messages and their editors keep their own direction in Arabic UI',async({page})=>{
        await page.goto('/');await expect(page.locator('#status')).toHaveText(copy.ar.Ready);await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
        await page.locator('#input').fill('STREAM');await page.locator('#input').press('Enter');await expect(page.locator('#output')).toContainText('First visible');await page.locator('#input').fill('Keep this English task');await page.locator('#btn-send').click();
        const content=page.locator('.pending-message p');await expect(content).toHaveText('Keep this English task');expect(await content.evaluate(element=>getComputedStyle(element).direction)).toBe('ltr');
        await page.locator('.pending-message button').first().click();const editor=page.locator('.pending-message textarea');await expect(editor).toHaveValue('Keep this English task');expect(await editor.evaluate(element=>getComputedStyle(element).direction)).toBe('ltr');
        await page.locator('#btn-cancel').click();await expect(page.locator('#status')).toHaveText(copy.ar.Ready);
    });
});

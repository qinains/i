import {test,expect} from './fixtures.mjs';
import {DEFAULT_CONFIGURATION} from '../config.js';

for(const [locale,language]of [['zh-CN','zh-CN'],['zh-HK','zh-TW'],['ja-JP','ja']])test(`first visit follows system ${locale}`,async({page})=>{
    await page.addInitScript(locale=>Object.defineProperty(navigator,'languages',{get:()=>[locale,'en-US']}),locale);
    await page.goto('/');await expect(page.locator('#welcome-connection')).toBeVisible();await expect(page.locator('html')).toHaveAttribute('lang',language);await expect(page.locator('#lang-select')).toHaveValue(language);
});

test('manual language preference wins over system language after reload',async({page})=>{
    await page.addInitScript(()=>Object.defineProperty(navigator,'languages',{get:()=>['zh-CN']}));
    await page.goto('/');await expect(page.locator('#welcome-connection')).toContainText('欢迎');
    await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('en');await page.reload();
    await expect(page.locator('#input')).toBeEnabled();await expect(page.locator('html')).toHaveAttribute('lang','en');await expect(page.locator('#lang-select')).toHaveValue('en');
});

test('first visit guides setup, remembers explicitly saved credentials and makes no model request',async({page})=>{
    const external=[];page.on('request',request=>{if(request.url().startsWith('https://model.invalid'))external.push(request.url());});
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();
    await expect(page.locator('#settings-modal')).toBeVisible();await expect(page.locator('#welcome-connection')).toBeVisible();
    await expect(page.locator('#remember-config')).toBeChecked();await expect(page.locator('#base-url')).toBeFocused();
    await expect(page.locator('#credential-privacy')).toContainText('WASM');
    await page.locator('#base-url').fill('https://model.invalid');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('ONBOARDING_FAKE_KEY');
    await page.locator('#save-config').click();await expect(page.locator('#settings-modal')).not.toBeVisible();
    await page.reload();await expect(page.locator('#input')).toBeEnabled();await expect(page.locator('#settings-modal')).not.toBeVisible();
    await page.locator('#settings-btn').click();await expect(page.locator('#api-key')).toHaveValue('ONBOARDING_FAKE_KEY');
    await expect(page.locator('#base-url')).toHaveValue('https://model.invalid');expect(external).toEqual([]);
});

test('welcome may be dismissed and remembering may be declined without saving credentials',async({page})=>{
    await page.goto('/');await expect(page.locator('#welcome-connection')).toBeVisible();await page.locator('#remember-config').uncheck();
    await page.locator('#base-url').fill('https://model.invalid');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('UNSAVED_FAKE_KEY');
    await page.locator('#save-config').click();await expect(page.locator('#settings-modal')).not.toBeVisible();await page.reload();
    await expect(page.locator('#input')).toBeEnabled();await expect(page.locator('#settings-modal')).not.toBeVisible();
    expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('UNSAVED_FAKE_KEY');await expect(page.locator('#api-key')).toHaveValue('');
});

test('welcome and credential privacy are translated in Chinese on narrow screens',async({page})=>{
    await page.setViewportSize({width:390,height:844});await page.goto('/');await expect(page.locator('#welcome-connection')).toBeVisible();
    await page.locator('[data-settings-tab=appearance]').click();await page.locator('#lang-select').selectOption('zh-CN');await page.locator('[data-settings-tab=connection]').click();
    await expect(page.locator('#welcome-connection')).toContainText('欢迎');await expect(page.locator('#credential-privacy')).toContainText('本地');
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});

test('closing welcome without setup does not force it open after reload',async({page})=>{
    await page.goto('/');await expect(page.locator('#welcome-connection')).toBeVisible();await page.locator('#close-settings').click();await page.reload();
    await expect(page.locator('#input')).toBeEnabled();await expect(page.locator('#settings-modal')).not.toBeVisible();
});

test('toggling Remember back on never saves before the Save button',async({page})=>{
    await page.goto('/');await expect(page.locator('#welcome-connection')).toBeVisible();
    await page.locator('#base-url').fill('https://model.invalid');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('SAVE_BUTTON_ONLY_FAKE');
    await page.locator('#remember-config').uncheck();await page.locator('#remember-config').check();
    await expect(page.locator('#settings-modal')).toBeVisible();expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('SAVE_BUTTON_ONLY_FAKE');
    await page.locator('#save-config').click();expect(await page.evaluate(()=>JSON.stringify(localStorage))).toContain('SAVE_BUTTON_ONLY_FAKE');
});

test('an existing saved connection without an active selection does not trigger welcome',async({page})=>{
    await page.addInitScript(settings=>localStorage.setItem('i_harness_profiles',JSON.stringify({profiles:{spare:settings},current:null})),{...DEFAULT_CONFIGURATION,base_url:'https://model.invalid',model:'fixture',api_key:'EXISTING_FAKE'});
    await page.goto('/');await expect(page.locator('#input')).toBeEnabled();await expect(page.locator('#settings-modal')).not.toBeVisible();
    await page.locator('#settings-btn').click();await expect(page.locator('#profile-select option[value=spare]')).toHaveCount(1);
});

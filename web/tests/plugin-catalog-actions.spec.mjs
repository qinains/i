import {test,expect} from './fixtures.mjs';
import {pluginPackage} from './plugin-fixtures.mjs';
import {createHash} from 'node:crypto';
import {CATALOG_URL} from '../plugins/catalog.js';
import {controlMetrics} from '../../site/control-contrast.mjs';

async function catalog(page,{holdDownload=false}={}){
    const bytes=Buffer.from(await pluginPackage()),sha256=createHash('sha256').update(bytes).digest('hex');
    const entry={id:'example.word-count',name:'Word count',version:'1.0.0',description:'Count words in workspace text files.',repository:'https://github.com/a-publisher-with-a-long-name/a-repository-with-a-long-name-for-plugin-source-review',source_revision:'a'.repeat(40),license:'MIT',package:{url:`https://qinains.github.io/i-plugins/packages/${sha256}.hplugin.zip`,sha256,size:bytes.length},permissions:{workspace:'read',storage:false}};
    let release;const gate=holdDownload?new Promise(resolve=>{release=resolve;}):Promise.resolve();
    await page.route(CATALOG_URL,route=>route.fulfill({contentType:'application/json',headers:{'access-control-allow-origin':'*'},body:JSON.stringify({schema_version:1,plugins:[entry]})}));
    await page.route(entry.package.url,async route=>{await gate;await route.fulfill({contentType:'application/zip',headers:{'access-control-allow-origin':'*'},body:bytes});});
    await page.goto('/?settings=plugins');await expect(page.locator('#plugin-catalog-load')).toBeEnabled();await page.locator('#plugin-catalog-load').click();
    const card=page.locator(`[data-catalog-plugin="${entry.id}"]`);await expect(card).toBeVisible();
    return {entry,card,release};
}
test('catalog card actions stay distinct, readable and accessible across themes, mobile and RTL',async({page,browserName})=>{
    const {entry,card}=await catalog(page),footer=card.locator('.plugin-catalog-actions'),source=footer.locator('a'),inspect=footer.locator('[data-catalog-inspect]');
    // Safari's default Tab cycle skips links; Option includes clickable items.
    const backToSource=browserName==='webkit'&&process.platform==='darwin'?'Alt+Shift+Tab':'Shift+Tab';
    await expect(source).toHaveAttribute('href',entry.repository);await expect(source).toHaveAttribute('title',entry.repository);await expect(source).toHaveAttribute('rel','noopener noreferrer');await expect(source).toHaveAttribute('referrerpolicy','no-referrer');
    for(const width of [1280,390])for(const theme of ['dark','light'])for(const language of ['en','ar']){
        await page.setViewportSize({width,height:844});await page.evaluate(async({theme,language})=>{document.documentElement.dataset.theme=theme;const {applyWorkspaceTranslations}=await import('/i18n.js');applyWorkspaceTranslations(language);},{theme,language});
        await expect(source).not.toContainText('github.com');await expect(inspect.locator('svg[aria-hidden=true]')).toHaveCount(1);await expect(inspect.locator('[data-i18n="Download and inspect"]')).not.toBeEmpty();
        const metrics=await footer.locator('button,a').evaluateAll(controlMetrics);for(const control of metrics){expect(control.ratio,`${width}/${theme}/${language} ${control.label}`).toBeGreaterThanOrEqual(4.5);expect(control.height).toBeGreaterThanOrEqual(44);}
        expect(await card.evaluate(element=>element.scrollWidth<=element.clientWidth)).toBe(true);
        const a=await source.boundingBox(),b=await inspect.boundingBox(),f=await footer.boundingBox();expect(a&&b&&f).toBeTruthy();
        expect(a.x+a.width<=b.x-8||b.x+b.width<=a.x-8||a.y+a.height<=b.y-8).toBe(true);
        if(width===390){expect(Math.abs(b.width-f.width)).toBeLessThanOrEqual(2);expect(b.y).toBeGreaterThanOrEqual(a.y+a.height+8);}
        await source.focus();await source.press('Tab');await expect(inspect).toBeFocused();await expect(inspect).toHaveCSS('outline-width','2px');await inspect.press(backToSource);await expect(source).toBeFocused();await expect(source).toHaveCSS('outline-width','2px');
    }
});
test('catalog primary action is disabled while inspection runs and never installs implicitly',async({page})=>{
    const {entry,card,release}=await catalog(page,{holdDownload:true});const inspect=card.locator('[data-catalog-inspect]');
    try{await inspect.click();await expect(inspect).toBeDisabled();await expect(inspect).toHaveCSS('cursor','not-allowed');await expect(page.locator(`[data-plugin-id="${entry.id}"]`)).toHaveCount(0);}
    finally{release();}
    await expect(page.locator('#plugin-preview')).toBeVisible();await expect(inspect).toBeEnabled();await expect(page.locator(`[data-plugin-id="${entry.id}"]`)).toHaveCount(0);await expect(page.locator('#plugin-install')).toBeVisible();
});

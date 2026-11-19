import assert from 'node:assert/strict';
import {chromium,webkit} from '../web/node_modules/playwright/index.mjs';
import {catalogs} from './assets/i18n.js';
import {languageOptions} from './assets/languages.js';

const origin=process.env.SITE_TEST_ORIGIN||'http://localhost:8090';
for(const [name,browserType] of Object.entries({chromium,webkit})){
    const browser=await browserType.launch({headless:true});
    try{
        const page=await browser.newPage({locale:'en-US',viewport:{width:1440,height:900}});
        const language=page.locator('#site-language');
        await page.goto(origin+'/docs/');
        await page.locator('html[lang=en]').waitFor();
        assert.equal(await page.locator('article h1').textContent(),'Documentation');
        assert.equal(await page.locator('.toc strong').textContent(),'On this page');
        const articleSize=await page.locator('article').evaluate(el=>parseFloat(getComputedStyle(el).fontSize));
        const navSize=await page.locator('.sidebar').evaluate(el=>parseFloat(getComputedStyle(el).fontSize));
        assert.ok(articleSize>=18&&navSize>=16);
        assert.equal(await page.locator('.sidebar nav a:not([data-site-i18n])').count(),0);
        const placeholders=page.locator('article [data-site-i18n], .sidebar [data-site-i18n], .toc [data-site-i18n]');
        for(const {code,dir} of languageOptions){
            await language.selectOption(code);
            assert.equal(await page.locator('html').getAttribute('dir'),dir);
            assert.equal(await page.title(),catalogs[code]['docs.page.index']+' · i');
            const labels=await placeholders.evaluateAll(items=>items.map(el=>[el.dataset.siteI18n,el.textContent]));
            for(const [key,value] of labels)assert.equal(value,catalogs[code][key],`${name}:${code}:${key}`);
        }
        await language.selectOption('en');
        await page.locator('#search').fill('connect a model');
        await page.locator('#search-results a[href="/docs/connect-model/"]').waitFor();
        assert.equal(await page.locator('#search-results a[href="/docs/connect-model/"]').textContent(),'Connect a model');
        await page.locator('article a[href="/docs/plugin-development/"]').click();
        const note=page.locator('.docs-language-notice');
        await note.waitFor();
        assert.equal(await page.title(),'Plugin development tutorial · i');
        assert.equal(await page.locator('article').getAttribute('lang'),'zh-CN');
        assert.equal((await page.locator('article h1').textContent()).replace(/#$/u,''),'从零编写 Rust 插件');
        for(const {code} of languageOptions){
            await language.selectOption(code);
            assert.equal(await note.isVisible(),code!=='zh-CN');
            assert.equal(await note.locator('span').textContent(),catalogs[code]['docs.original']);
            assert.equal(await page.locator('.source-link a').textContent(),catalogs[code]['docs.edit']);
        }
        await page.setViewportSize({width:320,height:800});
        await language.selectOption('ar');
        assert.equal(await page.locator('article').getAttribute('dir'),'ltr');
        assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
        await page.goto(origin+'/docs/');
        assert.equal(await page.locator('html').getAttribute('lang'),'ar');
        for(const {code} of languageOptions){
            await language.selectOption(code);
            assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`${name}:${code}: documentation overflow`);
        }
        await language.selectOption('ar');
        await page.goto(origin+'/docs/console/');
        assert.equal(await page.locator('article').getAttribute('lang'),'en');
        await page.locator('.docs-language-notice').waitFor();
        await language.selectOption('en');
        assert.equal(await page.locator('.docs-language-notice').isVisible(),false);
        const fontSize=selector=>page.locator(selector).first().evaluate(el=>parseFloat(getComputedStyle(el).fontSize));
        for(const width of [1440,320]){
            await page.setViewportSize({width,height:900});
            await page.goto(origin+'/');
            assert.equal(await fontSize('#search'),16);
            assert.equal(await fontSize('.plugin-copy > p:not(.eyebrow)'),articleSize);
            assert.equal(await fontSize('.boundary-list p'),articleSize);
            assert.equal(await fontSize('#client-browser .install-steps p'),articleSize);
            assert.equal(await fontSize('.docs-language-note'),15);
            const referenceLinks=await page.locator('.landing a[href^="/docs/"]:not(.button)').evaluateAll(links=>
                links.map(link=>[getComputedStyle(link).fontSize,getComputedStyle(link).textDecorationLine]));
            assert.ok(referenceLinks.length>=8);
            assert.ok(referenceLinks.every(([size,line])=>size==='16px'&&line.includes('underline')));
            for(const {code} of languageOptions){
                await language.selectOption(code);
                assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`${name}:${code}: home guide overflow at ${width}px`);
            }
        }
        await page.goto(origin+'/plugins/');
        assert.equal(await fontSize('#native-plugins .plugin-card p'),articleSize);
        assert.equal(await fontSize('#native-plugins .plugin-card a'),16);
        assert.equal(await fontSize('#search'),16);
        assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
        console.log(`PASS ${name}: all 17 documentation locales, localized search, source fallback, and consistent guides on home, docs, and plugins`);
    }finally{await browser.close();}
}

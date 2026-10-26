import assert from 'node:assert/strict';
import {mkdtemp,rm,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium,webkit} from '../web/node_modules/playwright/index.mjs';
import {languageOptions} from './assets/languages.js';
await import('./test-button-contrast.mjs');
await import('./test-select-layout.mjs');

const profile=await mkdtemp(join(tmpdir(),'lninl-site-'));
const origin=process.env.SITE_TEST_ORIGIN||'http://localhost:8090';
await mkdir(new URL('../dist/site-screenshots/',import.meta.url),{recursive:true});
try{
    for(const [name,browser]of Object.entries({chromium,webkit})){
        const context=await browser.launchPersistentContext(join(profile,name),{headless:true,locale:'en-US',viewport:{width:1440,height:1000}}),page=await context.newPage(),errors=[];
        page.on('pageerror',error=>errors.push(error.message));
        try{
            let releaseHome;
            const homeGate=new Promise(resolve=>{releaseHome=resolve;});
            await page.route('**/assets/home.js',async route=>{await homeGate;await route.continue();});
            await page.goto(origin+'/',{waitUntil:'commit'});await page.locator('.home-hero h1').waitFor();await page.locator('html[lang=en]').waitFor();
            assert.ok((await page.locator('.home-hero h1').textContent()).includes('Describe it.'));
            assert.equal(await page.locator('[data-client-tab=macos]').isDisabled(),true);releaseHome();
            for(const platform of ['macos','windows','linux','browser']){
                await page.locator(`[data-client-tab=${platform}]`).click();await page.locator(`#client-${platform}:visible`).waitFor();
                assert.equal(await page.locator('[data-client-tab][aria-selected=true]').count(),1);
                if(platform!=='browser'){const panel=page.locator(`#client-${platform}`);assert.equal(await panel.locator('.install-steps pre').count(),1);assert.equal(await panel.locator('[data-native-install]').evaluate(el=>el.open),false);assert.ok((await panel.locator('.install-steps code').textContent()).endsWith(platform==='windows'?'i.exe':'i'));assert.ok((await panel.locator('.client-download').getAttribute('href')).startsWith('https://github.com/qinains/i/releases'));}
            }
            await page.locator('[data-client-tab=browser]').focus();await page.keyboard.press('ArrowRight');assert.equal(await page.locator('[data-client-tab=macos]').getAttribute('aria-selected'),'true');await page.keyboard.press('Home');
            const modelRequests=[];page.on('request',request=>{if(request.method()==='POST')modelRequests.push(request.url());});
            assert.equal(await page.locator('#site-language option').count(),17);
            for(const {code:language,dir}of languageOptions){
                await page.locator('#site-language').selectOption(language);assert.equal(await page.locator('html').getAttribute('lang'),language);
                assert.equal(await page.locator('html').getAttribute('dir'),dir);
                assert.ok(await page.locator('.home-hero h1').textContent());
                for(const key of ['launch.description','launch.cta','connect.guide','security.summary','security.link','boundary.data.detail','boundary.credentials.detail','boundary.plugins.detail','boundary.preview.detail']){
                    const text=await page.locator(`[data-site-i18n="${key}"]`).textContent();
                    assert.ok(text.trim().length>5,`${name}:${language}:${key}:missing security copy`);
                    if(language==='en')assert.ok(!/[\u4e00-\u9fff]/u.test(text),`${key}:untranslated English`);
                }
                assert.equal(await page.locator('#security a[href="/docs/security/"]').count(),1);
                await page.locator('.launch-card').scrollIntoViewIfNeeded();
                await page.waitForFunction(()=>{const image=document.querySelector('.launch-card img');return image.complete&&image.naturalWidth>0;});
                assert.equal(await page.locator('.launch-card img').evaluate(image=>image.complete&&image.naturalWidth>0),true);
                assert.equal(new URL(await page.locator('.launch-card a').getAttribute('href')).hostname,'www.producthunt.com');
                await page.setViewportSize({width:390,height:844});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`${name}:${language}:mobile overflow`);
                for(const platform of ['macos','windows','linux','browser']){
                    await page.locator(`[data-client-tab=${platform}]`).click();assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`${name}:${language}:${platform}:mobile overflow`);
                }
                if(language==='ar'){
                    assert.equal(await page.locator('.demo-code').evaluate(element=>getComputedStyle(element).direction),'ltr');
                    await page.setViewportSize({width:320,height:780});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`${name}:Arabic 320px overflow`);
                    await page.screenshot({path:new URL(`../dist/site-screenshots/${name}-arabic-mobile.png`,import.meta.url).pathname,animations:'disabled'});
                }
            }
            await page.locator('#site-language').selectOption('zh-CN');await page.reload();await page.locator('html[lang="zh-CN"]').waitFor();
            await page.locator('[data-demo-task=refactor]').click();assert.ok((await page.locator('#demo-code').textContent()).includes('edit('));assert.equal(await page.locator('[data-demo-task=refactor]').getAttribute('aria-pressed'),'true');
            await page.locator('#site-language').selectOption('ja');assert.ok((await page.locator('#demo-prompt').textContent()).includes('関連コード'));
            await page.locator('[data-demo-task=plugin]').click();assert.ok((await page.locator('#demo-code').textContent()).includes('word_count'));
            assert.deepEqual(modelRequests,[]);
            await page.locator('#site-language').selectOption('zh-CN');await page.setViewportSize({width:1440,height:1000});
            await page.screenshot({path:new URL(`../dist/site-screenshots/${name}-home.png`,import.meta.url).pathname,fullPage:true,animations:'disabled'});
            await page.screenshot({path:new URL(`../dist/site-screenshots/${name}-hero.png`,import.meta.url).pathname,animations:'disabled'});
            let releaseSearch;
            const gate=new Promise(resolve=>{releaseSearch=resolve;});
            await page.route('**/search-index.json',async route=>{await gate;await route.continue();});
            await page.locator('#search').fill('harness-wasm-v1');await page.locator('#site-language').selectOption('en');releaseSearch();await page.locator('#search-results a').first().waitFor();
            const matches=await page.locator('#search-results a').evaluateAll(links=>links.map(link=>link.href));assert.equal(new Set(matches).size,matches.length);
            await page.locator('#site-language').selectOption('zh-CN');await page.locator('#search').fill('');
            await page.setViewportSize({width:390,height:844});await page.screenshot({path:new URL(`../dist/site-screenshots/${name}-home-mobile.png`,import.meta.url).pathname,fullPage:false,animations:'disabled'});
            await page.goto(origin+'/docs/plugin-development/');assert.ok(await page.locator('article').textContent().then(text=>text.includes('hplugin.zip')));
            assert.equal(await page.locator('a[rel=canonical]').count(),0);
            await page.setViewportSize({width:390,height:844});await page.reload();assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
            await page.locator('.docs-menu summary').first().click();await page.getByRole('navigation',{name:'文档',exact:true}).waitFor();
            await page.screenshot({path:new URL(`../dist/site-screenshots/${name}-docs-mobile.png`,import.meta.url).pathname,fullPage:false});
            await page.goto(origin+'/app/');await page.locator('#input:not(:disabled)').waitFor({timeout:20000});
            await page.locator('#settings-modal').waitFor({state:'visible',timeout:20000});await page.locator('#cors-help summary').click();assert.equal(await page.locator('#cors-help a[download]').count(),3);
            assert.deepEqual(errors,[]);console.log(`PASS ${name}: seventeen homepage languages, Arabic RTL, persisted choice, demo tasks without model requests, search, plugin docs, mobile overflow, /app/ WASM and extension links`);
        }finally{await context.close();}
        const system=await browser.launchPersistentContext(join(profile,name+'-system'),{headless:true,locale:'fr-FR',reducedMotion:'reduce',viewport:{width:320,height:780}});
        try{
            const fresh=await system.newPage();await fresh.addInitScript(()=>localStorage.setItem('lninl_site_language','INVALID'));
            await fresh.goto(origin+'/');await fresh.locator('html[lang=fr]').waitFor();
            assert.ok(await fresh.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`${name}:320px French overflow`);
            assert.equal(await fresh.locator('.terminal-cursor').evaluate(element=>getComputedStyle(element).animationName),'none');
            const before=await fresh.locator('header .brand span').evaluate(element=>getComputedStyle(element).color);assert.equal(before,'rgb(34, 211, 238)');
            await fresh.locator('#theme').click();assert.equal(await fresh.locator('html').getAttribute('data-theme'),'light');
            assert.equal(await fresh.locator('header .brand span').evaluate(element=>getComputedStyle(element).color),before);
            await fresh.reload();await fresh.locator('html[data-theme=light]').waitFor();
            await fresh.addInitScript(()=>{Storage.prototype.getItem=()=>{throw new Error('Unavailable');};Storage.prototype.setItem=()=>{throw new Error('Unavailable');};});
            await fresh.reload();await fresh.locator('html[lang=fr]').waitFor();await fresh.locator('#site-language').selectOption('de');await fresh.locator('html[lang=de]').waitFor();
            console.log(`PASS ${name}: system locale, invalid preference fallback, reduced motion, stable logo color, persisted theme, blocked storage`);
        }finally{await system.close();}
    }
}finally{await rm(profile,{recursive:true,force:true});}

import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {catalogs,detectSiteLanguage,resolveSiteLanguage,translate} from '../../site/assets/i18n.js';
import {languageCodes} from '../languages.js';

test('site language follows system preference with script-aware Chinese and safe fallbacks',()=>{
    for(const [languages,expected]of [[['zh-HK'],'zh-TW'],[['zh-Hans-TW'],'zh-CN'],[['fr-CA'],'fr'],[['es','ja-JP'],'es'],[['unknown','ja-JP'],'ja'],[['pt-BR'],'pt'],[['ar-SA'],'ar'],[['en-GB'],'en'],[[],'en']])assert.equal(detectSiteLanguage(languages),expected);
    assert.equal(resolveSiteLanguage('ko',['zh-CN']),'ko');
    assert.equal(resolveSiteLanguage('invalid',['de-DE']),'de');
    assert.equal(resolveSiteLanguage(null,['zh-CN']),'zh-CN');
});
test('all static home and shell translation markers exist in every catalog',async()=>{
    for(const file of ['home.html','install.html','template.html']){
        const source=await readFile(new URL('../../site/'+file,import.meta.url),'utf8');
        for(const match of source.matchAll(/data-site-i18n(?:-aria|-placeholder)?="([^"]+)"/gu))for(const [language,messages]of Object.entries(catalogs))assert.ok(messages[match[1]],`${file}:${language}:${match[1]}`);
    }
});
test('every website language provides all messages without silent English fallback',()=>{
    const keys=Object.keys(catalogs.en).sort();
    assert.deepEqual(Object.keys(catalogs).sort(),[...languageCodes].sort());
    for(const [language,messages]of Object.entries(catalogs)){
        assert.deepEqual(Object.keys(messages).sort(),keys,language);
        for(const key of keys){assert.equal(typeof messages[key],'string');assert.ok(messages[key].trim(),`${language}:${key}`);assert.equal(translate(language,key),messages[key]);}
    }
    assert.equal(translate('unknown','nav.docs'),catalogs.en['nav.docs']);
});

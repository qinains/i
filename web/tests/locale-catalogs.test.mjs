import assert from 'node:assert/strict';
import test from 'node:test';
import {readFile} from 'node:fs/promises';
import {copy} from '../ui-copy.js';
import {catalogs} from '../../site/assets/i18n.js';
const registry=JSON.parse(await readFile(new URL('../../locales/languages.json',import.meta.url),'utf8'));
const codes=registry.map(item=>item.code);
const tokens=text=>[...text.matchAll(/\{(?:[a-zA-Z_]\w*|\d*)\}/gu)].map(match=>match[0]).sort();
test('site and workspace expose exactly the shared seventeen-language registry',()=>{
    assert.equal(codes.length,17);assert.equal(new Set(codes).size,17);
    assert.deepEqual(Object.keys(catalogs).sort(),[...codes].sort());
    assert.deepEqual(Object.keys(copy).sort(),[...codes].sort());
    assert.deepEqual(registry.filter(item=>item.dir==='rtl').map(item=>item.code),['ar']);
});
test('native, workspace and site source catalogs are complete and preserve format placeholders',async()=>{
    for(const area of ['', 'web/', 'site/']){
        const base=JSON.parse(await readFile(new URL(`../../locales/${area}en.json`,import.meta.url),'utf8'));
        for(const code of codes){
            const messages=JSON.parse(await readFile(new URL(`../../locales/${area}${code}.json`,import.meta.url),'utf8'));
            assert.deepEqual(Object.keys(messages).sort(),Object.keys(base).sort(),`${area}${code}:keys`);
            for(const [key,value]of Object.entries(messages)){
                assert.equal(typeof value,'string',`${area}${code}:${key}`);assert.ok(value.trim());
                assert.deepEqual(tokens(value),tokens(base[key]),`${area}${code}:${key}:placeholders`);
            }
        }
    }
});
test('all static workspace translation markers have real catalog entries',async()=>{
    const source=await readFile(new URL('../index.html',import.meta.url),'utf8');
    for(const match of source.matchAll(/data-i18n(?:-aria-label|-title|-placeholder)?="([^"]+)"/gu))for(const code of codes)assert.ok(copy[code][match[1]],`${code}:${match[1]}`);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const root = new URL('../',import.meta.url);
test('PWA manifest and content-addressed asset list support relative deployment', async () => {
    const manifest = JSON.parse(await readFile(new URL('manifest.webmanifest',root)));
    assert.equal(manifest.start_url,'./'); assert.equal(manifest.scope,'./'); assert.equal(manifest.display,'standalone');
    const build = JSON.parse(await readFile(new URL('assets.json',root)));
    for (const asset of build.precache) {
        assert.ok(build.assets.includes(asset.url)); assert.ok(!asset.url.startsWith('/'));
        const bytes = await readFile(new URL(asset.url,root));
        assert.equal(createHash('sha256').update(bytes).digest('hex'),asset.sha256,asset.url);
        assert.ok(!/tests|node_modules|api\/|config\.json/u.test(asset.url));
    }
    assert.ok(build.assets.some(a => a.endsWith('.wasm')));
    assert.ok(build.assets.some(a => /snippets\/.*\/sse.js/u.test(a)));
});

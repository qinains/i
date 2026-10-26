import assert from 'node:assert/strict';
import test from 'node:test';
import {copy} from '../ui-copy.js';
import {readFile} from 'node:fs/promises';

test('every PWA status is translated in each supported UI language',async()=>{
    const source=await readFile(new URL('../pwa.js',import.meta.url),'utf8');
    const keys=[...source.matchAll(/'([^'\n]*(?:Update |Updating|Saving state|Checking open|Finish or stop|No update|PWA |Online\.|Offline:)[^'\n]*)'/gu)].map(match=>match[1]);
    assert.ok(keys.length>=16);
    for(const language of ['zh-CN','zh-TW','ja','ko','fr','de'])for(const key of keys)assert.ok(copy[language][key]&&copy[language][key]!==key,`${language}:${key}`);
});

test('connection guide and draggable control instructions cover every translated UI',()=>{
    const keys=[
        'First time? Get an API key and connect in 3 steps',
        '1. Create an API key on your provider’s official platform. API usage is billed separately from chat subscriptions.',
        '2. Copy the API URL and exact model ID, then paste the key only into the API key field.',
        'DeepSeek example: OpenAI-compatible (Chat Completions), API URL https://api.deepseek.com, model deepseek-flash. For other providers, use their own settings; do not mix keys, regions or plans.',
        '3. Test the connection, then Save changes. Testing sends a small paid API request. Direct browser access also requires CORS support.',
        'Step-by-step guide and troubleshooting →',
        'Drag to move; click to exit fullscreen',
    ];
    for(const language of ['zh-CN','zh-TW','ja','ko','fr','de'])for(const key of keys){
        assert.ok(typeof copy[language][key]==='string'&&copy[language][key].trim(),`${language}:${key}`);
        assert.notEqual(copy[language][key],key);
    }
});

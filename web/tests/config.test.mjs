import test from 'node:test';
import assert from 'node:assert/strict';
import {saveConfiguration, loadConfiguration, clearConfiguration, CONFIG_KEY} from '../config.js';
const settings = {base_url:'https://model.example/v1',model:'fixture',api_key:'SECRET',protocol:'openai',max_steps:12,max_tokens:4000,use_max_tokens:false,language:'zh-CN'};
function storage() {
    const values = new Map();
    return {getItem:key => values.get(key) ?? null,setItem:(key,value) => values.set(key,value),removeItem:key => values.delete(key)};
}
test('entire harness configuration roundtrips after explicit saving', () => {
    const store = storage();
    assert.equal(loadConfiguration(store),null);
    saveConfiguration(store,settings);
    assert.deepEqual(loadConfiguration(store),settings);
    assert.equal(JSON.parse(store.getItem(CONFIG_KEY)).version,1);
});
test('clearing saved config leaves unrelated origin storage untouched', () => {
    const store = storage(); store.setItem('other-app','preserve');
    saveConfiguration(store,settings); clearConfiguration(store);
    assert.equal(loadConfiguration(store),null);
    assert.equal(store.getItem('other-app'),'preserve');
});
test('corrupt or incompatible saved configurations never load raw credentials into errors', () => {
    const store = storage();
    for (const raw of ['SECRET',JSON.stringify({version:99,settings}),JSON.stringify({version:1,settings:{...settings,max_steps:0}})]) {
        store.setItem(CONFIG_KEY,raw);
        assert.throws(() => loadConfiguration(store), error => !error.message.includes('SECRET'));
    }
});
test('storage denial reports failure without pretending configuration was saved', () => {
    const store = {setItem:() => {throw new DOMException('denied','SecurityError');}};
    assert.throws(() => saveConfiguration(store,settings),/denied/);
});

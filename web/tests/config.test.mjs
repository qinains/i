import test from 'node:test';
import assert from 'node:assert/strict';
import {saveConfiguration, loadConfiguration, clearConfiguration, CONFIG_KEY,saveProfile,loadProfile,PROFILES_KEY,modelMaximum,modelSuggestions,DEFAULT_CONFIGURATION} from '../config.js';
const settings = {base_url:'https://model.example/v1',model:'fixture',api_key:'SECRET',protocol:'openai',max_tokens:4000,use_max_tokens:false,reasoning_effort:'default',language:'zh-CN',transport:'direct',stream:true,relay_token:''};
function storage() {
    const values = new Map();
    return {getItem:key => values.get(key) ?? null,setItem:(key,value) => values.set(key,value),removeItem:key => values.delete(key)};
}
test('new connections default to 64K while saved limits remain unchanged',()=>{
    assert.equal(DEFAULT_CONFIGURATION.max_tokens,65536);
    const store=storage();saveProfile(store,'existing',{...settings,max_tokens:8192});
    assert.equal(loadProfile(store,'existing').max_tokens,8192);
});
test('entire harness configuration roundtrips after explicit saving', () => {
    const store = storage();
    assert.equal(loadConfiguration(store),null);
    saveConfiguration(store,settings);
    assert.deepEqual(loadConfiguration(store),{...settings,custom_headers:''});
    assert.equal(JSON.parse(store.getItem(CONFIG_KEY)).version,2);
});
test('Anthropic protocol survives saved configuration and profile roundtrips',()=>{
    const store=storage(),native={...settings,protocol:'anthropic',base_url:'https://api.example/v1/messages'};
    saveConfiguration(store,native);assert.equal(loadConfiguration(store).protocol,'anthropic');
    saveProfile(store,'native',native);assert.equal(loadProfile(store,'native').protocol,'anthropic');
    assert.throws(()=>saveConfiguration(store,{...native,protocol:'messages'}));
});
test('Responses protocol and explicit reasoning strength survive local connection profiles',()=>{
    const store=storage();
    for(const reasoning_effort of ['default','none','low','medium','high','xhigh']){
        const config={...settings,protocol:'responses',reasoning_effort,max_tokens:65536};
        saveProfile(store,'responses',config);assert.equal(loadProfile(store,'responses').protocol,'responses');
        assert.equal(loadProfile(store,'responses').reasoning_effort,reasoning_effort);
    }
});
test('legacy configuration migrates to direct streaming without sending or changing key', () => {
    const store = storage();
    const {transport,stream,relay_token,...legacy} = settings;
    store.setItem(CONFIG_KEY,JSON.stringify({version:1,settings:legacy}));
    assert.deepEqual(loadConfiguration(store),{...settings,custom_headers:''});
});
test('clearing credentials includes saved profiles, not unrelated workspace metadata',()=>{
    const store=storage(); saveProfile(store,'default',settings);store.setItem('i_workspaces','keep');
    clearConfiguration(store);assert.equal(store.getItem(PROFILES_KEY),null);assert.equal(store.getItem('i_workspaces'),'keep');
});
test('profile loading validates stored data',()=>{
    const store=storage();store.setItem(PROFILES_KEY,JSON.stringify({profiles:{bad:{...settings,max_tokens:0}}}));
    assert.throws(()=>loadProfile(store,'bad'));
});
test('invalid transport and stream types are rejected', () => {
    for (const patch of [{transport:'extension'},{stream:'true'},{relay_token:123}]) {
        assert.throws(() => saveConfiguration(storage(),{...settings,...patch}));
    }
});
test('clearing saved config leaves unrelated origin storage untouched', () => {
    const store = storage(); store.setItem('other-app','preserve');
    saveConfiguration(store,settings); clearConfiguration(store);
    assert.equal(loadConfiguration(store),null);
    assert.equal(store.getItem('other-app'),'preserve');
});
test('corrupt or incompatible saved configurations never load raw credentials into errors', () => {
    const store = storage();
    for (const raw of ['SECRET',JSON.stringify({version:99,settings}),JSON.stringify({version:1,settings:{...settings,max_tokens:0}})]) {
        store.setItem(CONFIG_KEY,raw);
        assert.throws(() => loadConfiguration(store), error => !error.message.includes('SECRET'));
    }
});
test('old step settings are discarded without losing credentials or explicit token limits',()=>{
    const store=storage(),{reasoning_effort,...legacy}=settings;
    store.setItem(CONFIG_KEY,JSON.stringify({version:2,settings:{...legacy,max_steps:12}}));
    assert.deepEqual(loadConfiguration(store),{...settings,custom_headers:''});
    saveConfiguration(store,loadConfiguration(store));
    assert.ok(!store.getItem(CONFIG_KEY).includes('max_steps'));
});
test('thinking settings persist and reject unsupported strengths',()=>{
    const store=storage();
    for(const reasoning_effort of ['default','none','low','high','max']){
        saveConfiguration(store,{...settings,reasoning_effort});assert.equal(loadConfiguration(store).reasoning_effort,reasoning_effort);
    }
    assert.throws(()=>saveConfiguration(store,{...settings,reasoning_effort:'ultra'}));
});
test('maximum preset is restricted to documented official DeepSeek models',()=>{
    const config={...settings,base_url:'https://api.deepseek.com',model:'deepseek-flash'};
    assert.equal(modelMaximum(config),393216);
    assert.equal(modelMaximum({...config,model:'deepseek-v4-pro'}),393216);
    for(const patch of [{base_url:'https://api.deepseek.com.other.example'},{base_url:'https://proxy.example'},{protocol:'anthropic'},{model:'unknown'},{base_url:'http://api.deepseek.com'}])assert.equal(modelMaximum({...config,...patch}),null);
});
test('reference models do not depend on a selected model and are never guessed for other APIs',()=>{
    const config={...settings,base_url:'https://api.deepseek.com',model:''};
    assert.deepEqual(modelSuggestions(config),['deepseek-flash','deepseek-v4-pro']);
    assert.deepEqual(modelSuggestions({...config,base_url:'https://proxy.example'}),[]);
    assert.deepEqual(modelSuggestions({...config,protocol:'anthropic'}),[]);
});
test('storage denial reports failure without pretending configuration was saved', () => {
    const store = {setItem:() => {throw new DOMException('denied','SecurityError');}};
    assert.throws(() => saveConfiguration(store,settings),/denied/);
});

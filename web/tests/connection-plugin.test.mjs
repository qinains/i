import test from 'node:test';
import assert from 'node:assert/strict';
import {consumePairingFragment,connectionPluginDraft,configurationForManifest,validateConnectionPlugin,validateConnectionManifest,localHelperRequest,visiblePluginModels,loginAccount} from '../connection-plugin.js';
import {DEFAULT_CONFIGURATION,saveConfiguration,loadConfiguration,saveProfile,loadProfile} from '../config.js';
import {fetchModel} from '../platform.js';
import {requestCompletion} from '../transport.js';

const token='a'.repeat(48),url='http://127.0.0.1:12345';
const fragment=pair=>`#connection-plugin=${Buffer.from(JSON.stringify(pair)).toString('base64url')}`;
const config={...DEFAULT_CONFIGURATION,base_url:url+'/v1/responses',api_key:token,protocol:'responses',transport:'plugin',model:'actual-model'};
test('reauthorization reuses retained registration unless the user explicitly adds an account',()=>{
    const status={active:'first',accounts:[{id:'first',signed_in:true},{id:'second',signed_in:false}]};
    assert.equal(loginAccount(status),'first');assert.equal(loginAccount(status,'second'),'second');
    assert.equal(loginAccount({...status,active:null}),'first');
    assert.equal(loginAccount(status,'__new__'),undefined);assert.equal(loginAccount({accounts:[]}),undefined);
});
test('explicit pairing fragments are removed before parsing and create isolated drafts',()=>{
    const selected={...DEFAULT_CONFIGURATION,api_key:'existing-key',model:'existing-model'};
    const location={hash:fragment({url,token}),pathname:'/app/',search:'?lang=en'};let stripped;
    const pair=consumePairingFragment(location,{replaceState:(_state,_title,path)=>{stripped=path;}});
    assert.equal(stripped,'/app/?lang=en');
    const draft=connectionPluginDraft(selected,pair);
    assert.equal(draft.base_url,url+'/v1/responses');assert.equal(draft.model,'');assert.equal(draft.api_key,token);assert.equal(draft.transport,'plugin');
    assert.equal(selected.api_key,'existing-key');assert.equal(selected.model,'existing-model');
    assert.equal(consumePairingFragment({...location,hash:'#other=x'},{}),null);
});
test('pairing and configurations reject normalized loopback aliases and unsafe endpoints',()=>{
    for(const base of ['http://localhost:12345','http://127.1:12345','http://2130706433:12345','http://0x7f000001:12345','http://127.0.0.1.evil:12345','https://127.0.0.1:12345','http://user@127.0.0.1:12345','http://127.0.0.1:12345/?x=1','http://127.0.0.1:12345/#x','http://127.0.0.1:12345/path']){
        let stripped=false;
        assert.throws(()=>consumePairingFragment({hash:fragment({url:base,token}),pathname:'/app/',search:''},{replaceState:()=>{stripped=true;}}));assert.ok(stripped);
    }
    for(const patch of [{protocol:'openai'},{stream:false},{custom_headers:'{}'},{api_key:'short'},{base_url:'https://evil.example/v1/responses'}])assert.throws(()=>validateConnectionPlugin({...config,...patch}));
});
test('local capabilities roundtrip with Remember and profiles, without OAuth credentials',()=>{
    const values=new Map(),storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};
    saveConfiguration(storage,config);assert.deepEqual(loadConfiguration(storage),config);
    saveProfile(storage,'local',config);assert.deepEqual(loadProfile(storage,'local'),config);
    assert.throws(()=>saveConfiguration(storage,{...config,base_url:'https://evil.example/v1/responses'}));
});
test('local metadata uses capability header, omits cookies and rejects redirects',async()=>{
    const original=globalThis.fetch;let seen;
    globalThis.fetch=async(endpoint,options)=>{seen={endpoint,options};return Response.json({active:null,accounts:[]});};
    try{await localHelperRequest(config,'status');assert.equal(seen.endpoint,url+'/v1/status');assert.equal(seen.options.headers['X-I-Local-Token'],token);assert.equal(seen.options.headers.authorization,undefined);assert.equal(seen.options.credentials,'omit');assert.equal(seen.options.redirect,'error');await assert.rejects(localHelperRequest(config,'../../evil'));await localHelperRequest(config,'cancel',{body:{}});assert.equal(seen.options.method,'POST');assert.equal(seen.options.body,'{}');}
    finally{globalThis.fetch=original;}
});
test('manifests are limited typed metadata and never executable or arbitrary instructions',()=>{
    const manifest={format_version:1,kind:'connection',id:'openai.chatgpt',name:'ChatGPT',protocol:'responses'};
    assert.deepEqual(validateConnectionManifest(manifest),manifest);
    for(const patch of [{format_version:2},{kind:'wasm'},{id:'https://evil.example'},{protocol:'openai'},{name:'x\nsecret'},{oauth_token:'secret'},{url:'https://evil.example'}])assert.throws(()=>validateConnectionManifest({...manifest,...patch}));
});
test('Anthropic connection plugin manifests adapt only unsaved pairing drafts',()=>{
    const manifest={format_version:1,kind:'connection',id:'anthropic.console',name:'Claude Console',protocol:'anthropic'};
    assert.deepEqual(validateConnectionManifest(manifest),manifest);
    assert.throws(()=>configurationForManifest(config,manifest));
    const converted=configurationForManifest(config,manifest,{draft:true});
    assert.equal(converted.protocol,'anthropic');assert.equal(converted.base_url,url+'/v1/messages');assert.equal(converted.api_key,token);
    assert.equal(validateConnectionPlugin(converted),url);
    assert.throws(()=>validateConnectionPlugin({...converted,base_url:url+'/v1/responses'}));
    assert.throws(()=>validateConnectionPlugin({...config,base_url:url+'/v1/messages'}));
    const values=new Map(),storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};
    saveConfiguration(storage,converted);assert.deepEqual(loadConfiguration(storage),converted);
});
test('Anthropic local connection uses only capability authentication and native SSE parsing',async()=>{
    const original=globalThis.fetch;let seen;
    const event=(type,fields={})=>'data: '+JSON.stringify({type,...fields})+'\n\n';
    const sse=event('message_start',{message:{id:'msg_local',type:'message',role:'assistant',content:[],stop_reason:null,usage:{input_tokens:1,output_tokens:0}}})+event('content_block_start',{index:0,content_block:{type:'text',text:''}})+event('content_block_delta',{index:0,delta:{type:'text_delta',text:'local Claude'}})+event('content_block_stop',{index:0})+event('message_delta',{delta:{stop_reason:'end_turn'},usage:{output_tokens:2}})+event('message_stop');
    globalThis.fetch=async(endpoint,options)=>{seen={endpoint,options};return new Response(sse,{headers:{'content-type':'text/event-stream'}});};
    try{
        const result=JSON.parse(await fetchModel(url+'/v1/messages',JSON.stringify({stream:true,messages:[],tools:[]}),token,new AbortController(),{transport:'plugin',protocol:'anthropic'}));
        assert.equal(result.content[0].text,'local Claude');assert.equal(seen.options.headers['X-I-Local-Token'],token);
        for(const header of ['authorization','x-api-key','anthropic-version','anthropic-dangerous-direct-browser-access'])assert.equal(seen.options.headers[header],undefined);
    }finally{globalThis.fetch=original;}
});
test('model catalog only exposes real list-visible models',()=>{
    assert.deepEqual(visiblePluginModels({models:[{slug:'real',display_name:'Real',visibility:'list'},{slug:'hidden',visibility:'hide'},{slug:'unknown'}]}),[{slug:'real',display_name:'Real',visibility:'list'}]);
});
test('local model transport never sends a Bearer token or body credentials and enforces streaming',async()=>{
    const original=globalThis.fetch;let seen;
    globalThis.fetch=async(endpoint,options)=>{seen={endpoint,options};return new Response(['response.created','response.completed'].map(type=>'data: '+JSON.stringify({type,response:{id:'r',object:'response',status:type==='response.created'?'in_progress':'completed',output:[]}})+'\n\n').join(''),{headers:{'content-type':'text/event-stream'}});};
    try{
        await fetchModel(config.base_url,JSON.stringify({stream:true,api_key:'oauth-secret',apiKey:'oauth-secret',tools:[]}),token,new AbortController(),{transport:'plugin',protocol:'responses'});
        assert.equal(seen.options.headers['X-I-Local-Token'],token);assert.equal(seen.options.headers.authorization,undefined);assert.ok(!seen.options.body.includes('oauth-secret'));
        for(const [endpoint,body,options]of [[config.base_url,'{}',{protocol:'responses'}],['http://localhost:12345/v1/responses','{"stream":true}',{protocol:'responses'}],[config.base_url,'{"stream":true}',{protocol:'openai'}],[config.base_url,'{"stream":true}',{protocol:'responses',customHeaders:'{}'}]])await assert.rejects(fetchModel(endpoint,body,token,new AbortController(),{transport:'plugin',...options}));
        await assert.rejects(requestCompletion({endpoint:'http://127.1:12345/v1/responses',body:{stream:true},apiKey:token,transport:'plugin',protocol:'responses'}));
    }finally{globalThis.fetch=original;}
});

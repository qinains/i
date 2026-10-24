import test from 'node:test';
import assert from 'node:assert/strict';
import {makeRule,permissionOrigins} from '../../extensions/cors-helper/rules.js';
import {readPublicConnection,validateEndpoint} from '../../extensions/cors-helper/endpoint.js';

test('autofill reads only public endpoint metadata, never settings or credential fields',()=>{
    const original=globalThis.document,reads=[];
    globalThis.document={documentElement:{getAttribute(name){reads.push(name);return {'data-model-api-url':'https://api.example/chat/completions','data-model-transport':'direct'}[name];}}};
    try{assert.deepEqual(readPublicConnection(),{endpoint:'https://api.example/chat/completions',transport:'direct'});assert.deepEqual(reads,['data-model-api-url','data-model-transport']);}finally{globalThis.document=original;}
});
test('empty and invalid manual endpoints have safe guidance, not native URL errors',()=>{
    for(const value of ['', 'not-a-url', 'https://user:SECRET@api.example', 'https://api.example/?key=SECRET']){
        assert.throws(()=>validateEndpoint(1,'https://app.example',value),error=>error.message.includes('API')&&!error.message.includes('SECRET')&&!error.message.includes('Invalid URL'));
    }
    assert.equal(validateEndpoint(1,'https://app.example',' https://api.example/chat/completions '),'https://api.example/chat/completions');
});
test('navigation to another page origin cannot enable a stale popup request',async()=>{
    const original=globalThis.chrome;let touched=false;
    globalThis.chrome={tabs:{get:async()=>({url:'https://other.example/'})},permissions:{contains:async()=>{touched=true;return true;}}};
    try{const {enableForTab}=await import('../../extensions/cors-helper/actions.js?origin-test');await assert.rejects(enableForTab(1,'https://api.example/chat/completions',{expectedOrigin:'https://app.example'}));assert.equal(touched,false);}finally{globalThis.chrome=original;}
});
for(const stage of ['permission','badge'])test(`navigation during ${stage} rolls back the stale grant`,async()=>{
    const original=globalThis.chrome;let url='https://app.example/',rules=[],saved={},badge='';
    globalThis.chrome={
        tabs:{get:async()=>({url})},
        permissions:{contains:async()=>{if(stage==='permission')url='https://other.example/';return true;}},
        declarativeNetRequest:{isRegexSupported:async()=>({isSupported:true}),updateSessionRules:async({removeRuleIds=[],addRules=[]})=>{rules=rules.filter(rule=>!removeRuleIds.includes(rule.id)).concat(addRules);},getSessionRules:async()=>rules},
        storage:{local:{set:async value=>Object.assign(saved,value),remove:async key=>{delete saved[key];}}},
        action:{setBadgeText:async({text})=>{badge=text;if(stage==='badge'&&text==='ON')url='https://other.example/';}}
    };
    try{
        const {enableForTab}=await import(`../../extensions/cors-helper/actions.js?race-${stage}`);
        await assert.rejects(enableForTab(1,'https://api.example/chat/completions',{expectedOrigin:'https://app.example'}));
        assert.deepEqual(rules,[]);assert.deepEqual(saved,{});assert.equal(badge,'');
    }finally{globalThis.chrome=original;}
});

test('CORS rules target one tab, exact API URL and exact requesting origin',()=>{
    const rule=makeRule(42,'http://localhost:8080/','https://api.example/v1/chat/completions');
    assert.deepEqual(rule.condition.tabIds,[42]);
    assert.equal(new RegExp(rule.condition.regexFilter).test('https://api.example/v1/chat/completions'),true);
    for(const url of ['https://api.example/v1/chat/completions/other','https://api.example.evil/v1/chat/completions'])assert.equal(new RegExp(rule.condition.regexFilter).test(url),false);
    const headers=rule.action.responseHeaders;
    assert.equal(headers.find(item=>item.header==='access-control-allow-origin').value,'http://localhost:8080');
    assert.match(headers.find(item=>item.header==='access-control-allow-headers').value,/authorization/);
    assert.equal(rule.action.requestHeaders,undefined);
    assert.deepEqual(permissionOrigins('http://localhost:8080','https://api.example/v1/chat/completions'),['http://localhost/*','https://api.example/*']);
});
test('CORS configuration rejects opaque/insecure origins, credentials and invalid tab IDs',()=>{
    for(const origin of ['file:///tmp/index.html','http://remote.example','https://u:p@app.example','https://app.example/#fragment'])assert.throws(()=>makeRule(1,origin,'https://api.example/chat/completions'));
    for(const target of ['http://api.example/chat/completions','https://user:secret@api.example/v1','https://api.example/#x'])assert.throws(()=>makeRule(1,'https://app.example',target));
    assert.throws(()=>makeRule(-1,'https://app.example','https://api.example'));
});

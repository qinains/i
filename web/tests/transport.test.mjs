import test from 'node:test';
import assert from 'node:assert/strict';
import {normalizeEndpoint,validateCustomHeaders,requestCompletion} from '../transport.js';
test('invalid API URLs return a recognizable safe configuration error',()=>{
    for(const value of ['', 'not-a-url', 'api.example.com', 'https://user:URL_SECRET@api.example', 'https://api.example/?key=URL_SECRET']){
        assert.throws(()=>normalizeEndpoint(value),error=>error.code==='invalid_api_url'&&error.message.includes('Settings')&&!error.message.includes('URL_SECRET'));
    }
});
test('endpoint normalization accepts base and full endpoint identically',()=>{
    assert.equal(normalizeEndpoint('https://api.example/v1/'),'https://api.example/v1/chat/completions');
    assert.equal(normalizeEndpoint('https://api.example/v1/chat/completions'),'https://api.example/v1/chat/completions');
    for(const url of ['http://api.example/v1','https://u:p@api.example','https://api.example/?key=x']) assert.throws(()=>normalizeEndpoint(url));
});
test('custom headers validate and cannot replace credentials or routing',()=>{
    assert.deepEqual(validateCustomHeaders('{"X-Client":"test"}'),{'x-client':'test'});
    for(const raw of ['oops','[]','{"Authorization":"bad"}','{"X-Harness-Upstream":"bad"}','{"Origin":"bad"}','{"x":1}']) assert.throws(()=>validateCustomHeaders(raw));
});
test('Anthropic endpoints and auth header reservations',()=>{
    for(const base of ['https://api.example','https://api.example/v1/','https://api.example/v1/messages']) assert.equal(normalizeEndpoint(base,'anthropic'),'https://api.example/v1/messages');
    assert.throws(()=>normalizeEndpoint('https://api.example','unknown'));
    for(const name of ['x-api-key','anthropic-version','anthropic-dangerous-direct-browser-access']) assert.throws(()=>validateCustomHeaders(JSON.stringify({[name]:'bad'})));
});
test('Anthropic direct completion forwards native JSON and dedicated browser headers without retry',async()=>{
    const original=globalThis.fetch;let requests=0;
    const body={model:'fixture',system:[],messages:[{role:'user',content:[{type:'text',text:'Hi'}]}],max_tokens:5,stream:false};
    try {
        globalThis.fetch=async(url,options)=>{
            requests++;assert.equal(url,'https://api.example/v1/messages');
            assert.deepEqual(JSON.parse(options.body),body);assert.equal(options.headers.authorization,undefined);
            assert.equal(options.headers['x-api-key'],'KEY');assert.equal(options.headers['anthropic-version'],'2023-06-01');assert.equal(options.headers['anthropic-dangerous-direct-browser-access'],'true');
            assert.equal(options.redirect,'error');assert.equal(options.credentials,'omit');
            return new Response(JSON.stringify({type:'message',role:'assistant',content:[{type:'text',text:'Hi'}],stop_reason:'end_turn'}),{headers:{'content-type':'application/json'}});
        };
        const result=JSON.parse(await requestCompletion({endpoint:'https://api.example',protocol:'anthropic',body,apiKey:'KEY'}));
        assert.equal(result.type,'message');assert.equal(result.choices,undefined);assert.equal(requests,1);
        globalThis.fetch=async()=>{requests++;throw new Error('network KEY');};
        await assert.rejects(requestCompletion({endpoint:'https://api.example',protocol:'anthropic',body,apiKey:'KEY'}),error=>error.message==='network [redacted]');
        assert.equal(requests,2);
    } finally {globalThis.fetch=original;}
});

import test from 'node:test';
import assert from 'node:assert/strict';
import {makeRule,permissionOrigins} from '../../extensions/cors-helper/rules.js';

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

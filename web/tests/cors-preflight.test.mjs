import test from 'node:test';
import assert from 'node:assert/strict';
import {preflightResponse,createPreflightController} from '../../extensions/cors-helper/preflight.js';
import {makeRule} from '../../extensions/cors-helper/rules.js';
const config={origin:'https://app.example',endpoint:'https://api.example/chat/completions'};
const request={url:config.endpoint,method:'OPTIONS',headers:{Origin:config.origin,'Access-Control-Request-Method':'POST','Access-Control-Request-Headers':'authorization, content-type'}};
test('synthetic preflight is exact URL + origin + POST only, with no credential values',()=>{
    const response=preflightResponse(config,request);assert.equal(response.responseCode,204);assert.equal(response.body,'');assert.ok(!JSON.stringify(response).includes('SECRET'));
    for(const update of [{method:'POST',postData:'SECRET'},{url:config.endpoint+'/other'},{headers:{...request.headers,Origin:'https://other.example'}},{headers:{...request.headers,'Access-Control-Request-Method':'DELETE'}},{headers:{...request.headers,'Access-Control-Request-Private-Network':'true'}},{headers:{...request.headers,'Access-Control-Request-Headers':'x-header\r\nSECRET'}}])assert.equal(preflightResponse(config,{...request,...update}),null);
    assert.equal(preflightResponse(config,{...request,headers:{...request.headers,'Access-Control-Request-Headers':'\r\nauthorization'}}),null);
});
function fixture(){
    let url=config.origin+'/',detaches=0,event,detached,updated;const commands=[];
    const api={tabs:{get:async()=>({url}),onRemoved:{addListener(){}},onUpdated:{addListener(fn){updated=fn;}}},permissions:{contains:async()=>true},declarativeNetRequest:{getSessionRules:async()=>[makeRule(1,url,config.endpoint)]},debugger:{
        attach:async()=>{},detach:async()=>{detaches++;detached({tabId:1});},sendCommand:async(target,method,params)=>{commands.push({method,params});},onEvent:{addListener(fn){event=fn;}},onDetach:{addListener(fn){detached=fn;}}
    }};
    const controller=createPreflightController(api);return {api,controller,commands,emit:(...args)=>event(...args),navigate:next=>{url=next;updated(1,{url:next});},detached:()=>detached({tabId:1}),get detaches(){return detaches;}};
}
test('debugger is off initially, only intercepts Preflight and never buffers the POST',async()=>{
    const f=fixture();assert.deepEqual(f.controller.status(1),{active:false,endpoint:undefined});
    await f.controller.enable(1,config.endpoint,config.origin);
    assert.equal(f.commands[0].method,'Fetch.enable');assert.deepEqual(f.commands[0].params.patterns,[{urlPattern:config.endpoint,requestStage:'Request'}]);
    f.emit({tabId:1},'Fetch.requestPaused',{requestId:'one',request});await new Promise(resolve=>setTimeout(resolve,0));
    assert.equal(f.commands.at(-1).method,'Fetch.fulfillRequest');assert.equal(f.commands.at(-1).params.responseCode,204);
    f.emit({tabId:1},'Fetch.requestPaused',{requestId:'two',request:{...request,method:'POST',postData:'SECRET'}});await new Promise(resolve=>setTimeout(resolve,0));
    assert.deepEqual(f.commands.at(-1),{method:'Fetch.continueRequest',params:{requestId:'two'}});assert.ok(!JSON.stringify(f.commands).includes('SECRET'));
    await f.controller.disable(1);assert.equal(f.controller.status(1).active,false);assert.equal(f.detaches,1);
});
test('navigation and user debugger cancellation revoke repair without automatic reattach',async()=>{
    const f=fixture();await f.controller.enable(1,config.endpoint,config.origin);f.navigate('https://other.example/');await f.controller.disable(1);assert.equal(f.controller.status(1).active,false);
    const other=fixture();await other.controller.enable(1,config.endpoint,config.origin);other.detached();assert.equal(other.controller.status(1).active,false);
});
test('repair refuses missing normal rule, permission and a navigation during attach',async()=>{
    const f=fixture();f.api.declarativeNetRequest.getSessionRules=async()=>[];await assert.rejects(f.controller.enable(1,config.endpoint,config.origin));
    const denied=fixture();denied.api.permissions.contains=async()=>false;await assert.rejects(denied.controller.enable(1,config.endpoint,config.origin));
    const race=fixture();race.api.debugger.attach=async()=>race.navigate('https://other.example/');await assert.rejects(race.controller.enable(1,config.endpoint,config.origin));assert.equal(race.controller.status(1).active,false);assert.equal(race.detaches,1);
});
test('detach without an onDetach event clears stale state, but active detach failure stays visible',async()=>{
    const f=fixture();await f.controller.enable(1,config.endpoint,config.origin);
    f.api.debugger.detach=async()=>{throw new Error('Debugger is not attached to the tab with id: 1.');};
    await f.controller.disable(1);assert.equal(f.controller.status(1).active,false);
    await f.controller.enable(1,config.endpoint,config.origin);
    f.api.debugger.detach=async()=>{throw new Error('Detach failed');};
    await assert.rejects(f.controller.disable(1));
});
test('an old paused request cannot stop a newly authorized debugger session',async()=>{
    const f=fixture();await f.controller.enable(1,config.endpoint,config.origin);
    const get=f.api.tabs.get,send=f.api.debugger.sendCommand;let release;
    f.api.tabs.get=()=>new Promise(resolve=>{release=resolve;});
    f.api.debugger.sendCommand=async(target,method,params)=>{
        if(params?.requestId==='old')throw new Error('Invalid InterceptionId');
        return send(target,method,params);
    };
    f.emit({tabId:1},'Fetch.requestPaused',{requestId:'old',request});
    f.api.tabs.get=get;await f.controller.disable(1);await f.controller.enable(1,config.endpoint,config.origin);
    release({url:config.origin+'/'});await new Promise(resolve=>setTimeout(resolve,0));
    assert.equal(f.controller.status(1).active,true);assert.equal(f.detaches,1);
});

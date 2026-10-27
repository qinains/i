import test from 'node:test';
import assert from 'node:assert/strict';
import {runAnalysis,validateAnalysisSpec,narrowAnalysisSpec} from '../analysis.js';
const spec={prompt:'Summarize verifiable lessons.',selection:{roles:['user','assistant','tool'],start:0,end:null,max_messages:40,max_bytes:48000}};
test('generic analysis passes only the explicit bounded specification to the host',async()=>{
    let received;const history=[{role:'user',content:'retained'}],before=structuredClone(spec);
    const agent={analyze:async value=>{received=JSON.parse(value);return 'Independent report';},history};
    assert.equal(await runAnalysis({agent,spec,signal:new AbortController().signal}),'Independent report');
    assert.deepEqual(received,spec);assert.deepEqual(spec,before);assert.deepEqual(history,[{role:'user',content:'retained'}]);
    assert.equal(Object.hasOwn(received,'api_key'),false);assert.equal(Object.hasOwn(received,'tools'),false);
});
test('analysis specification rejects implicit system access, credentials and unbounded selections',()=>{
    for(const patch of [{prompt:''},{api_key:'secret'},{tools:[]},{selection:{...spec.selection,roles:['system']}},{selection:{...spec.selection,max_messages:0}},{selection:{...spec.selection,max_bytes:Infinity}},{selection:{...spec.selection,start:-1}},{selection:{...spec.selection,end:0,start:1}},{selection:{...spec.selection,roles:['assistant','assistant']}},{selection:{...spec.selection,private_state:true}}])assert.throws(()=>validateAnalysisSpec({...spec,...patch}));
});
test('scope narrowing cannot expand a declaration or invoke the model on denied roles',async()=>{
    const restricted={...spec,selection:{...spec.selection,roles:['user']}},before=structuredClone(restricted);let calls=0;
    assert.deepEqual(narrowAnalysisSpec(restricted,'all'),restricted);
    assert.deepEqual(narrowAnalysisSpec(restricted,'user').selection.roles,['user']);
    const agent={analyze:async()=>{calls++;return 'report';}};
    await assert.rejects(async()=>runAnalysis({agent,spec:narrowAnalysisSpec(restricted,'assistant')}),/Invalid analysis specification/);
    assert.throws(()=>narrowAnalysisSpec(restricted,'tool'),/Invalid analysis specification/);
    assert.throws(()=>narrowAnalysisSpec(restricted,'system'),/Invalid analysis specification/);
    assert.deepEqual(restricted,before);assert.equal(calls,0);
});
test('analysis cancellation reaches the host and discards any late report',async()=>{
    const controller=new AbortController();let finish,cancelled=0;
    const agent={analyze:()=>new Promise(resolve=>{finish=resolve;}),cancel:()=>{cancelled++;}};
    const pending=runAnalysis({agent,spec,signal:controller.signal});controller.abort();finish('late');
    await assert.rejects(pending,{name:'AbortError'});assert.equal(cancelled,1);
    await assert.rejects(runAnalysis({agent,spec,signal:controller.signal}),{name:'AbortError'});assert.equal(cancelled,1);
});
test('analysis rejects invalid result shapes and detaches cancellation when complete',async()=>{
    for(const result of [null,{},'', ' '.repeat(3)])await assert.rejects(runAnalysis({agent:{analyze:async()=>result},spec}),/invalid result/);
    const controller=new AbortController();let cancelled=0;
    await runAnalysis({agent:{analyze:async()=>'report',cancel:()=>{cancelled++;}},spec,signal:controller.signal});controller.abort();assert.equal(cancelled,0);
});
test('analysis result byte limit matches the native 128000-byte boundary',async()=>{
    const atLimit='中'.repeat(42666)+'aa';assert.equal(new TextEncoder().encode(atLimit).length,128000);
    assert.equal(await runAnalysis({agent:{analyze:async()=>atLimit},spec}),atLimit);
    await assert.rejects(runAnalysis({agent:{analyze:async()=>atLimit+'a'},spec}),/invalid result/);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import {readResponses} from '../responses-sse.js';
const event=(type,rest={})=>`event: ${type}\ndata: ${JSON.stringify({type,...rest})}\n\n`;
const reader=value=>new ReadableStream({start(controller){for(const byte of new TextEncoder().encode(value))controller.enqueue(Uint8Array.of(byte));controller.close();}}).getReader();
function stream({name='write',status='completed',text=false}={}) {
    const reasoning={id:'rs_1',type:'reasoning',summary:[{type:'summary_text',text:'Plan'}],encrypted_content:'OPAQUE'};
    const item=text?{id:'msg_1',type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text:'你好',annotations:[]}]}:{id:'fc_1',type:'function_call',call_id:'call_1',name,arguments:'{"path":"a","content":"ok"}',status:'completed'};
    return event('response.created',{response:{id:'resp_1',object:'response',status:'in_progress',output:[]}})
        +event('response.output_item.added',{output_index:0,item:{...reasoning,summary:[],encrypted_content:null}})
        +event('response.reasoning_summary_part.added',{output_index:0,item_id:'rs_1',summary_index:0,part:{type:'summary_text',text:''}})
        +event('response.reasoning_summary_text.delta',{output_index:0,item_id:'rs_1',summary_index:0,delta:'Plan'})
        +event('response.output_item.done',{output_index:0,item:reasoning})
        +event('response.output_item.added',{output_index:1,item:text?{...item,status:'in_progress',content:[]}:{...item,status:'in_progress',arguments:''}})
        +(text?event('response.content_part.added',{output_index:1,item_id:item.id,content_index:0,part:{type:'output_text',text:'',annotations:[]}})+event('response.output_text.delta',{output_index:1,item_id:item.id,content_index:0,delta:'你好'}):event('response.function_call_arguments.delta',{output_index:1,item_id:item.id,delta:item.arguments}))
        +event('response.output_item.done',{output_index:1,item})
        +event('response.'+status,{response:{id:'resp_1',object:'response',status,output:[reasoning,item],incomplete_details:status==='incomplete'?{reason:'max_output_tokens'}:null}});
}
test('Responses SSE reconstructs completed native output and only displays readable summaries',async()=>{
    const reasoning=[],progress=[];const result=await readResponses(reader(stream()),{onReasoning:text=>reasoning.push(text),onProgress:value=>progress.push(value)});
    assert.equal(result.output[0].encrypted_content,'OPAQUE');assert.equal(result.output[1].arguments,'{"path":"a","content":"ok"}');
    assert.equal(reasoning.join(''),'Plan');assert.ok(!JSON.stringify(progress).includes('OPAQUE'));assert.ok(progress.some(value=>value.phase==='tool'&&value.name==='write'));
});
test('Responses UTF-8 text streams before completion without duplication',async()=>{
    const chunks=[];const result=await readResponses(reader(stream({text:true})),{onDelta:text=>chunks.push(text)});
    assert.equal(chunks.join(''),'你好');assert.equal(result.output[1].content[0].text,'你好');
});
test('Responses incomplete, failed, unsupported, duplicate and truncated streams fail closed',async()=>{
    const complete=stream();
    for(const raw of [stream({status:'incomplete'}),stream({status:'failed'}),stream({name:'shell'}),complete.slice(0,complete.lastIndexOf('event: response.completed')),complete+event('response.completed',{response:{status:'completed'}}),complete.replace('"output_index":1','"output_index":3'),complete.replace('"output_index":1,"item_id":"fc_1"','"output_index":1,"item_id":"wrong"'),event('error',{message:'SECRET'})]) await assert.rejects(readResponses(reader(raw)));
});
test('Responses native tools are validated atomically and plugin tools need explicit schemas',async()=>{
    await assert.rejects(readResponses(reader(stream({name:'p_fixture_tool'}))));
    assert.equal((await readResponses(reader(stream({name:'p_fixture_tool'})),{allowedTools:['p_fixture_tool']})).output[1].name,'p_fixture_tool');
});
test('Responses terminal output must match completed items and argument deltas',async()=>{
    await assert.rejects(readResponses(reader(stream().replace('"arguments":""','"arguments":"forged"'))));
    const raw=stream(),index=raw.lastIndexOf('event: response.completed');
    await assert.rejects(readResponses(reader(raw.slice(0,index)+raw.slice(index).replace('"name":"write"','"name":"read"'))));
});
test('Responses byte limit and cancellation abort parsing without returning pending calls',async()=>{
    await assert.rejects(readResponses(reader(stream()),{maxBytes:10}));
    const controller=new AbortController();controller.abort();await assert.rejects(readResponses(reader(stream()),{signal:controller.signal}));
});
test('Responses progress is delivered before the terminal response is available',async()=>{
    const raw=stream({text:true}),index=raw.indexOf('event: response.output_item.done',raw.indexOf('"id":"msg_1"'));
    let source;const bytes=new TextEncoder();
    const r=new ReadableStream({start(controller){source=controller;controller.enqueue(bytes.encode(raw.slice(0,index)));}}).getReader();
    let visible='';const result=readResponses(r,{onDelta:text=>{visible+=text;}});
    await new Promise(resolve=>setTimeout(resolve,0));assert.equal(visible,'你好');
    source.enqueue(bytes.encode(raw.slice(index)));source.close();assert.equal((await result).status,'completed');
});
test('Responses interleaved parallel function calls retain each call id and argument buffer',async()=>{
    const one={id:'fc_a',type:'function_call',call_id:'a',name:'read',arguments:'{"files":[{"path":"a"}]}',status:'completed'};
    const two={id:'fc_b',type:'function_call',call_id:'b',name:'list',arguments:'{}',status:'completed'};
    const raw=event('response.created',{response:{id:'resp_parallel',object:'response',status:'in_progress'}})
        +[one,two].map((item,output_index)=>event('response.output_item.added',{output_index,item:{...item,arguments:'',status:'in_progress'}})).join('')
        +event('response.function_call_arguments.delta',{output_index:1,item_id:two.id,delta:two.arguments})
        +event('response.function_call_arguments.delta',{output_index:0,item_id:one.id,delta:one.arguments})
        +[two,one].map(item=>event('response.output_item.done',{output_index:item===one?0:1,item})).join('')
        +event('response.completed',{response:{object:'response',id:'resp_parallel',status:'completed',output:[one,two]}});
    assert.deepEqual((await readResponses(reader(raw))).output,[one,two]);
});

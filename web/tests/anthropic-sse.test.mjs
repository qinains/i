import test from 'node:test';
import assert from 'node:assert/strict';
import {readAnthropicMessage} from '../anthropic-sse.js';
const event=(type,rest={})=>`event: ${type}\ndata: ${JSON.stringify({type,...rest})}\n\n`;
export function messageStream({tool=true,stop=tool?'tool_use':'end_turn'}={}) {
    return event('message_start',{message:{id:'msg_test',type:'message',role:'assistant',content:[],stop_reason:null,usage:{input_tokens:1,output_tokens:0}}})
        +event('content_block_start',{index:0,content_block:{type:'thinking',thinking:''}})
        +event('content_block_delta',{index:0,delta:{type:'thinking_delta',thinking:'private'}})
        +event('content_block_delta',{index:0,delta:{type:'signature_delta',signature:'signed'}})
        +event('content_block_stop',{index:0})
        +event('content_block_start',{index:1,content_block:tool?{type:'tool_use',id:'call_1',name:'write',input:{}}:{type:'text',text:''}})
        +event('content_block_delta',{index:1,delta:tool?{type:'input_json_delta',partial_json:'{"path":"a.txt","content":"ok"}'}:{type:'text_delta',text:'done'}})
        +event('content_block_stop',{index:1})
        +event('message_delta',{delta:{stop_reason:stop,stop_sequence:null},usage:{output_tokens:2}})
        +event('message_stop');
}
const reader=value=>new ReadableStream({start(controller){for(const byte of new TextEncoder().encode(value))controller.enqueue(Uint8Array.of(byte));controller.close();}}).getReader();
test('readable Anthropic thinking is separate from signatures and redacted blocks',async()=>{
    const chunks=[];
    const raw=messageStream({tool:false}).replace('"thinking":""','"thinking":"initial "')
        .replace('event: message_delta',event('content_block_start',{index:2,content_block:{type:'redacted_thinking',data:'HIDDEN_DATA'}})+event('content_block_stop',{index:2})+'event: message_delta');
    await readAnthropicMessage(reader(raw),{onReasoning:text=>chunks.push(text)});
    assert.equal(chunks.join(''),'initial private');assert.ok(!chunks.join('').includes('signed'));
});
test('Anthropic SSE reconstructs tools and opaque thinking/signature across byte fragments',async()=>{
    const result=await readAnthropicMessage(reader(messageStream()));
    assert.equal(result.type,'message');
    assert.deepEqual(result.content,[{type:'thinking',thinking:'private',signature:'signed'},{type:'tool_use',id:'call_1',name:'write',input:{path:'a.txt',content:'ok'}}]);
    assert.equal(result.stop_reason,'tool_use');
    assert.deepEqual(result.usage,{input_tokens:1,output_tokens:2});
});
test('Anthropic SSE text deltas notify but tools wait until complete',async()=>{
    const deltas=[];
    const result=await readAnthropicMessage(reader(messageStream({tool:false})),{onDelta:text=>deltas.push(text)});
    assert.deepEqual(deltas,['done']);assert.equal(result.content[1].text,'done');
});
test('Anthropic reports thinking and tool progress without exposing their contents',async()=>{
    const progress=[];
    await readAnthropicMessage(reader(messageStream()),{onProgress:value=>progress.push(value)});
    assert.ok(progress.some(value=>value.phase==='reasoning'&&value.characters===7));
    assert.ok(progress.some(value=>value.phase==='tool'&&value.name==='write'&&value.characters>0));
    assert.ok(!JSON.stringify(progress).includes('private'));
    assert.ok(!JSON.stringify(progress).includes('signed'));
    assert.ok(!JSON.stringify(progress).includes('a.txt'));
});
test('Anthropic initial text and multiple blocks stream exactly the normalized final text',async()=>{
    const stream=event('message_start',{message:{type:'message',role:'assistant',content:[],stop_reason:null}})
        +event('content_block_start',{index:0,content_block:{type:'text',text:'prefix '}})
        +event('content_block_delta',{index:0,delta:{type:'text_delta',text:'one'}})+event('content_block_stop',{index:0})
        +event('content_block_start',{index:1,content_block:{type:'text',text:'two'}})+event('content_block_stop',{index:1})
        +event('message_delta',{delta:{stop_reason:'end_turn'}})+event('message_stop');
    const chunks=[];const result=await readAnthropicMessage(reader(stream),{onDelta:text=>chunks.push(text)});
    assert.equal(chunks.join(''),result.content.map(block=>block.text).join('\n'));
});
test('Anthropic SSE fails closed on truncation, error, bad JSON, bad finish and block order',async()=>{
    const original=messageStream();
    for(const value of [original.slice(0,original.lastIndexOf('event: message_stop')),original.replace('"stop_reason":"tool_use"','"stop_reason":"max_tokens"'),original.replace('\\"content\\":\\"ok\\"','\\"content\\":'),original.replace('"index":0','"index":2'),original+event('message_stop'),event('error',{error:{message:'bad'}}),original.replace('"name":"write"','"name":"shell"')]) {
        await assert.rejects(readAnthropicMessage(reader(value)));
    }
});
test('Anthropic SSE respects byte limit and cancellation',async()=>{
    await assert.rejects(readAnthropicMessage(reader(messageStream()),{maxBytes:10}));
    const controller=new AbortController();controller.abort();
    await assert.rejects(readAnthropicMessage(reader(messageStream()),{signal:controller.signal}));
});
test('Anthropic no-input tools accept the initial empty object only when no JSON deltas arrived',async()=>{
    const start=event('message_start',{message:{id:'empty_tool',type:'message',role:'assistant',content:[],stop_reason:null}})
        +event('content_block_start',{index:0,content_block:{type:'tool_use',id:'download_1',name:'download',input:{}}});
    const end=event('content_block_stop',{index:0})+event('message_delta',{delta:{stop_reason:'tool_use'}})+event('message_stop');
    const result=await readAnthropicMessage(reader(start+end));assert.deepEqual(result.content[0].input,{});
    for(const partial_json of ['', '{', '[]', 'null'])await assert.rejects(readAnthropicMessage(reader(start+event('content_block_delta',{index:0,delta:{type:'input_json_delta',partial_json}})+end)));
});

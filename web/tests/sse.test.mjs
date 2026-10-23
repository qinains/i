import test from 'node:test';
import assert from 'node:assert/strict';
import {readChatCompletion} from '../sse.js';
const frame = (delta,finish_reason=null) => `data: ${JSON.stringify({choices:[{index:0,delta,finish_reason}]})}\r\n\r\n`;
test('readable reasoning streams separately from the final answer',async()=>{
    const thought=[],answer=[];
    await readChatCompletion(reader(frame({reasoning_content:'Look '})+frame({reasoning_content:'closely'})+frame({content:'Done'})+frame({},'stop')+'data: [DONE]\n\n'),{onReasoning:text=>thought.push(text),onDelta:text=>answer.push(text)});
    assert.deepEqual(thought,['Look ','closely']);assert.deepEqual(answer,['Done']);
});
function reader(text, single=false) {
    const bytes = new TextEncoder().encode(text);
    return new ReadableStream({start(c) { if (single) for (const b of bytes) c.enqueue(Uint8Array.of(b)); else c.enqueue(bytes); c.close(); }}).getReader();
}
test('single-byte UTF8, CRLF, heartbeat, multiline data, usage and deltas', async () => {
    let shown = '';
    const raw = ': heartbeat\r\n\r\ndata: {"choices":\r\ndata: [{"index":0,"delta":{"content":"你"},"finish_reason":null}]}\r\n\r\n'
        +frame({content:'好',reasoning_content:'opaque'})+frame({},'stop')+'data: {"choices":[],"usage":{"total_tokens":3}}\n\ndata: [DONE]\n\n';
    const out = await readChatCompletion(reader(raw,true),{onDelta:t => shown += t});
    assert.equal(shown,'你好'); assert.equal(out.choices[0].message.content,'你好');
    assert.equal(out.choices[0].message.reasoning_content,'opaque'); assert.equal(out.usage.total_tokens,3);
});
test('interleaved tools are assembled without dispatching them', async () => {
    const raw = frame({tool_calls:[{index:0,id:'call_',type:'function',function:{name:'write',arguments:'{"p'}},{index:1,id:'b',type:'function',function:{name:'read',arguments:'{}'}}]})
        +frame({tool_calls:[{index:0,id:'123',function:{arguments:'ath":"x"}'}}]})+frame({},'tool_calls')+'data: [DONE]\n\n';
    const out = await readChatCompletion(reader(raw),{});
    assert.equal(out.choices[0].message.tool_calls[0].function.arguments,'{"path":"x"}');
    assert.equal(out.choices[0].message.tool_calls[1].id,'b');
    assert.equal(out.choices[0].message.tool_calls[0].id,'call_123');
});
test('truncated, conflicting, error and unsupported streams fail closed', async () => {
    for (const raw of [frame({content:'partial'}),frame({},'stop'),frame({})+'data: [DONE]\n\n',
        frame({},'length')+'data: [DONE]\n\n','data: {"error":{"message":"bad"}}\n\n',
        'data: {"choices":[{"index":1,"delta":{}}]}\n\n',
        frame({tool_calls:[{index:0,type:'function'}]})+frame({tool_calls:[{index:0,type:'other'}]}),
        frame({},'stop')+frame({content:'late'})+'data: [DONE]\n\n']) {
        await assert.rejects(readChatCompletion(reader(raw),{}));
    }
});
test('response byte limit and abort enforced', async () => {
    await assert.rejects(readChatCompletion(reader(frame({content:'long'})),{maxBytes:4}),/limit/i);
    const controller = new AbortController(); controller.abort();
    await assert.rejects(readChatCompletion(reader(''),{signal:controller.signal}),/abort/i);
});
test('incomplete finish reasons identify the upstream reason and never return pending tools',async()=>{
    for(const [reason,hint]of [['length','token'],['content_filter','filter'],['insufficient_system_resource','resource'],['aborted','interrupted'],['vendor_unknown','unsupported']]){
        const raw=frame({tool_calls:[{index:0,id:'pending',type:'function',function:{name:'write',arguments:'{"path":"index.html","content":"not committed"}'}}]})+frame({},reason)+'data: [DONE]\n\n';
        await assert.rejects(readChatCompletion(reader(raw)),error=>error.message.includes(`finish_reason=${reason}`)&&error.message.toLowerCase().includes(hint)&&error.message.includes('not executed'));
    }
});
test('reasoning and tool-only streams report safe progress before a length failure',async()=>{
    const progress=[],shown=[];
    const raw=frame({reasoning_content:'private reasoning'})
        +frame({tool_calls:[{index:0,id:'pending',function:{name:'write',arguments:'{"content":"PRIVATE'}}]})
        +frame({},'length')+'data: [DONE]\n\n';
    await assert.rejects(readChatCompletion(reader(raw,true),{onDelta:text=>shown.push(text),onProgress:value=>progress.push(value)}),/finish_reason=length/);
    assert.deepEqual(shown,[]);
    assert.deepEqual(progress,[{phase:'reasoning',characters:17},{phase:'tool',name:'write',characters:19}]);
    assert.ok(!JSON.stringify(progress).includes('private'));
    assert.ok(!JSON.stringify(progress).includes('PRIVATE'));
});

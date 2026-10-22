import test from 'node:test';
import assert from 'node:assert/strict';
import {readChatCompletion} from '../sse.js';
const frame = (delta,finish_reason=null) => `data: ${JSON.stringify({choices:[{index:0,delta,finish_reason}]})}\r\n\r\n`;
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

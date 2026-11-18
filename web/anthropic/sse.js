const LIMIT = 48 * 1024 * 1024;
function finishError(value){
    const reason=typeof value==='string'&&/^[a-z0-9_.-]{1,80}$/iu.test(value)?value:'invalid';
    const hints=new Map([
        ['length','Output token or context limit reached (some models include reasoning in this budget). Check Maximum output tokens in Settings > Model connection, or split the task. Streaming does not increase this limit.'],
        ['content_filter','The upstream content filter stopped the response.'],
        ['insufficient_system_resource','The upstream model ran out of resources. Retry manually when ready.'],
        ['aborted','The upstream generation was interrupted.'],
    ]);
    return new Error(`Incomplete model response (finish_reason=${reason}): ${hints.get(reason)??'Unsupported upstream finish reason; check provider compatibility.'} Pending tool calls were not executed.`);
}

export async function readChatCompletion(reader, {signal,onDelta = () => {},onReasoning = () => {},onProgress = () => {},maxBytes = LIMIT} = {}) {
    const decoder = new TextDecoder('utf-8',{fatal:true});
    let pending = '', data = [], size = 0, done = false, finish = null, usage;
    const message = {role:'assistant',content:''};
    const tools = new Map();
    function process() {
        if (!data.length) return;
        const value = data.join('\n'); data = [];
        if (done) throw new Error('Event after stream completion');
        if (value === '[DONE]') { done = true; return; }
        const event = JSON.parse(value);
        if (!event || event.error) throw new Error('Model stream error');
        if (event.usage != null) usage = event.usage;
        if (!Array.isArray(event.choices)) throw new Error('Invalid stream choices');
        if (!event.choices.length) return;
        if (event.choices.length !== 1 || event.choices[0].index !== 0) throw new Error('Only choice index 0 is supported');
        if (finish) throw new Error('Delta after finish_reason');
        const choice = event.choices[0], delta = choice.delta;
        if (!delta || typeof delta !== 'object' || Array.isArray(delta)) throw new Error('Invalid stream delta');
        if (delta.role != null && delta.role !== 'assistant') throw new Error('Invalid assistant role');
        for (const field of ['content','reasoning_content','refusal']) {
            if (delta[field] == null) continue;
            if (typeof delta[field] !== 'string') throw new Error('Invalid stream text');
            message[field] = (message[field] ?? '') + delta[field];
            if (field === 'content') onDelta(delta[field]);
            if (field === 'reasoning_content' && delta[field]) onReasoning(delta[field]);
            if(delta[field] && field!=='refusal')onProgress({phase:field==='content'?'text':'reasoning',characters:message[field].length});
        }
        if (delta.tool_calls != null) {
            if (!Array.isArray(delta.tool_calls)) throw new Error('Invalid tool fragments');
            for (const fragment of delta.tool_calls) {
                if (!Number.isInteger(fragment.index) || fragment.index < 0 || fragment.index >= 1000) throw new Error('Invalid tool index');
                let tool = tools.get(fragment.index);
                if (!tool) { tool = {id:'',type:'function',function:{name:'',arguments:''}}; tools.set(fragment.index,tool); }
                if (fragment.id != null) {
                    if (typeof fragment.id !== 'string') throw new Error('Invalid tool ID');
                    tool.id += fragment.id;
                }
                if (fragment.type != null && fragment.type !== 'function') throw new Error('Invalid tool type');
                if (fragment.function != null) {
                    for (const field of ['name','arguments']) if (fragment.function[field] != null) {
                        if (typeof fragment.function[field] !== 'string') throw new Error('Invalid tool fragment');
                        tool.function[field] += fragment.function[field];
                    }
                    onProgress({phase:'tool',name:tool.function.name,characters:tool.function.arguments.length});
                }
            }
        }
        if (choice.finish_reason != null) {
            if (!['stop','tool_calls'].includes(choice.finish_reason)) throw finishError(choice.finish_reason);
            finish = choice.finish_reason;
        }
    }
    function text(value) {
        pending += value;
        let index;
        while ((index = pending.indexOf('\n')) !== -1) {
            const line = pending.slice(0,index).replace(/\r$/u,''); pending = pending.slice(index+1);
            if (!line) process();
            else if (line.startsWith('data:')) data.push(line.slice(5).replace(/^ /u,''));
            else if (line === 'data') data.push('');
        }
    }
    try {
        for (;;) {
            signal?.throwIfAborted();
            const part = await reader.read();
            signal?.throwIfAborted();
            if (part.done) break;
            size += part.value.byteLength;
            if (size > maxBytes) throw new Error('Response exceeds byte limit');
            text(decoder.decode(part.value,{stream:true}));
        }
        text(decoder.decode());
        if (pending || data.length || !done || !finish) throw new Error('Incomplete model stream');
        if (tools.size) {
            if (finish !== 'tool_calls') throw new Error('Tools require tool_calls finish_reason');
            message.tool_calls = [...tools].sort(([a],[b]) => a-b).map(([index,tool],position) => {
                if (index !== position || !tool.id || !tool.function.name) throw new Error('Incomplete tool call');
                const args = JSON.parse(tool.function.arguments);
                if (!args || typeof args !== 'object' || Array.isArray(args)) throw new Error('Invalid tool arguments');
                return tool;
            });
            if (new Set(message.tool_calls.map(tool => tool.id)).size !== tools.size) throw new Error('Duplicate tool ID');
        } else if (finish === 'tool_calls') throw new Error('Missing tool calls');
        return {choices:[{index:0,message,finish_reason:finish}],...(usage == null ? {} : {usage})};
    } catch (error) { await reader.cancel().catch(() => {}); throw error; }
}

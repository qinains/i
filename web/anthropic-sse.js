const LIMIT=48*1024*1024;
const TOOLS=new Set(['read','write','edit','view_image','list','download','glob','search']);
const object=value=>value!==null && typeof value==='object' && !Array.isArray(value);
function require(condition,message='Invalid Anthropic stream') {if(!condition)throw new Error(message);}

export async function readAnthropicMessage(reader,{signal,onDelta=()=>{},onReasoning=()=>{},onProgress=()=>{},maxBytes=LIMIT,allowedTools=[]}={}) {
    const tools=new Set([...TOOLS,...allowedTools]);
    const decoder=new TextDecoder('utf-8',{fatal:true});
    let pending='',data=[],eventName='',size=0,message=null,active=null,finished=false,done=false,textBlocks=0;
    const ids=new Set();
    function process() {
        if(!data.length){eventName='';return;}
        const value=JSON.parse(data.join('\n'));data=[];
        require(object(value) && typeof value.type==='string' && (!eventName || eventName===value.type));eventName='';
        require(!done,'Event after Anthropic message_stop');
        if(value.type==='error')throw new Error('Anthropic stream error');
        if(value.type==='ping')return;
        if(value.type==='message_start') {
            require(!message && object(value.message) && value.message.type==='message' && value.message.role==='assistant' && Array.isArray(value.message.content) && value.message.content.length===0 && value.message.stop_reason==null);
            message=value.message;return;
        }
        require(message,'Missing Anthropic message_start');
        if(value.type==='message_stop') {
            require(finished && !active,'Incomplete Anthropic message');done=true;return;
        }
        require(!finished,'Event after Anthropic message_delta');
        if(value.type==='message_delta') {
            if(value.delta?.stop_reason==='max_tokens')throw new Error('Incomplete model response (stop_reason=max_tokens): Output token limit reached. Check Maximum output tokens in Settings > Model connection, or split the task. Pending tool calls were not executed.');
            require(!active && object(value.delta) && ['end_turn','stop_sequence','tool_use'].includes(value.delta.stop_reason),'Incomplete Anthropic stop_reason');
            require((value.delta.stop_reason==='tool_use')===(ids.size>0),'Incomplete Anthropic tool batch');
            Object.assign(message,value.delta);
            if(value.usage!=null){require(object(value.usage));message.usage={...message.usage,...value.usage};}
            finished=true;return;
        }
        require(Number.isInteger(value.index) && value.index>=0 && value.index<1000,'Invalid Anthropic block index');
        if(value.type==='content_block_start') {
            require(!active && value.index===message.content.length && object(value.content_block));
            const block=value.content_block;
            switch(block.type) {
            case 'text':{
                require(typeof block.text==='string');
                const initial=(textBlocks++?'\n':'')+block.text;if(initial)onDelta(initial);
                break;
            }
            case 'thinking':require(typeof block.thinking==='string' && (block.signature==null || typeof block.signature==='string'));block.signature??='';if(block.thinking)onReasoning(block.thinking);onProgress({phase:'reasoning',characters:block.thinking.length});break;
            case 'redacted_thinking':require(typeof block.data==='string');onProgress({phase:'reasoning',characters:0});break;
            case 'tool_use':
                require(typeof block.id==='string' && block.id.length>0 && !ids.has(block.id) && tools.has(block.name) && object(block.input) && Object.keys(block.input).length===0 && ids.size<128,'Invalid or unsupported Anthropic tool');
                ids.add(block.id);onProgress({phase:'tool',name:block.name,characters:0});break;
            default:throw new Error('Unsupported Anthropic content block');
            }
            active={index:value.index,block,json:'',hasJsonDelta:false};return;
        }
        require(active && active.index===value.index,'Anthropic block order mismatch');
        if(value.type==='content_block_stop') {
            if(active.block.type==='tool_use') {
                // A no-input tool may keep its initial {}; any received JSON delta must parse.
                if(active.hasJsonDelta)active.block.input=JSON.parse(active.json);
                require(object(active.block.input),'Invalid Anthropic tool JSON');
            }
            message.content.push(active.block);active=null;return;
        }
        require(value.type==='content_block_delta' && object(value.delta));
        const delta=value.delta,block=active.block;
        if(delta.type==='text_delta' && block.type==='text') {
            require(typeof delta.text==='string');block.text+=delta.text;onDelta(delta.text);
            onProgress({phase:'text',characters:block.text.length});
        } else if(delta.type==='input_json_delta' && block.type==='tool_use') {
            require(typeof delta.partial_json==='string');active.hasJsonDelta=true;active.json+=delta.partial_json;
            onProgress({phase:'tool',name:block.name,characters:active.json.length});
        } else if(delta.type==='thinking_delta' && block.type==='thinking') {
            require(typeof delta.thinking==='string');block.thinking+=delta.thinking;
            if(delta.thinking)onReasoning(delta.thinking);
            onProgress({phase:'reasoning',characters:block.thinking.length});
        } else if(delta.type==='signature_delta' && block.type==='thinking') {
            require(typeof delta.signature==='string');block.signature+=delta.signature;
        } else throw new Error('Unsupported Anthropic content delta');
    }
    function text(value) {
        pending+=value;
        let index;
        while((index=pending.indexOf('\n'))!==-1) {
            const line=pending.slice(0,index).replace(/\r$/u,'');pending=pending.slice(index+1);
            if(!line)process();
            else if(line.startsWith('data:'))data.push(line.slice(5).replace(/^ /u,''));
            else if(line==='data')data.push('');
            else if(line.startsWith('event:'))eventName=line.slice(6).trim();
        }
    }
    try {
        for(;;) {
            signal?.throwIfAborted();
            const part=await reader.read();signal?.throwIfAborted();
            if(part.done)break;
            size+=part.value.byteLength;if(size>maxBytes)throw new Error('Response exceeds byte limit');
            text(decoder.decode(part.value,{stream:true}));
        }
        text(decoder.decode());
        require(!pending && !data.length && !eventName && done && finished && !active,'Incomplete Anthropic stream');
        return message;
    } catch(error) {await reader.cancel().catch(()=>{});throw error;}
}

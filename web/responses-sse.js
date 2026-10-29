const LIMIT=48*1024*1024;
const TOOLS=['read','write','edit','view_image','list','download','glob','search'];
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
function check(condition,message='Invalid Responses stream'){if(!condition)throw new Error(message);}
function same(a,b){
    if(a===b)return true;
    if(Array.isArray(a))return Array.isArray(b)&&a.length===b.length&&a.every((value,index)=>same(value,b[index]));
    return object(a)&&object(b)&&Object.keys(a).length===Object.keys(b).length&&Object.keys(a).every(key=>Object.hasOwn(b,key)&&same(a[key],b[key]));
}
export async function readResponses(reader,{signal,onDelta=()=>{},onReasoning=()=>{},onProgress=()=>{},maxBytes=LIMIT,allowedTools=[]}={}) {
    const tools=new Set([...TOOLS,...allowedTools]),items=[],ids=new Set(),calls=new Set();
    const decoder=new TextDecoder('utf-8',{fatal:true});
    let pending='',data=[],eventName='',size=0,start=null,result=null,visibleParts=0;
    function validateItem(item,complete){
        check(object(item)&&typeof item.id==='string'&&item.id.length>0);
        if(item.type==='function_call'){
            check(typeof item.call_id==='string'&&item.call_id.length>0&&tools.has(item.name)&&typeof item.arguments==='string','Invalid or unsupported Responses tool');
            check(!complete||item.status==='completed','Incomplete Responses function call');
            if(complete)check(object(JSON.parse(item.arguments)),'Invalid Responses tool arguments');
        }else if(item.type==='message')check(item.role==='assistant'&&Array.isArray(item.content)&&(!complete||item.status==='completed'));
        else if(item.type==='reasoning')check(Array.isArray(item.summary));
        else throw new Error('Unsupported Responses output item');
    }
    function process(){
        if(!data.length){eventName='';return;}
        const value=JSON.parse(data.join('\n'));data=[];
        check(object(value)&&typeof value.type==='string'&&(!eventName||eventName===value.type));eventName='';
        check(!result,'Event after Responses completion');
        const type=value.type;
        if(type==='error'||type==='response.failed')throw new Error('Responses API stream failed. Pending tool calls were not executed.');
        if(type==='response.incomplete'){
            const reason=value.response?.incomplete_details?.reason;
            const hint=reason==='max_output_tokens'?'Output token limit reached. Check Maximum output tokens in Settings > Model connection.':reason==='content_filter'?'The provider stopped this response with a content filter.':'The provider stopped this response before completion.';
            throw new Error(`Incomplete Responses response: ${hint} Pending tool calls were not executed.`);
        }
        if(type==='response.created'){
            check(!start&&value.response?.object==='response'&&value.response.status==='in_progress'&&typeof value.response.id==='string');start=value.response;return;
        }
        check(start,'Missing response.created');
        if(type==='response.in_progress'){check(value.response?.id===start.id);return;}
        if(type==='response.metadata'){
            check(value.response_id===start.id&&Object.hasOwn(value,'metadata'),'Invalid Responses metadata');
            return;
        }
        if(type==='response.completed'){
            const response=value.response;
            check(response?.object==='response'&&response.id===start.id&&response.status==='completed'&&Array.isArray(response.output)&&response.output.length===items.length&&items.every((item,index)=>item.done&&same(item.done,response.output[index])),'Incomplete or mismatched Responses output');
            result=response;return;
        }
        check(Number.isInteger(value.output_index)&&value.output_index>=0&&value.output_index<1000,'Invalid Responses output index');
        if(type==='response.output_item.added'){
            check(value.output_index===items.length);validateItem(value.item,false);
            check(!ids.has(value.item.id),'Duplicate Responses item id');ids.add(value.item.id);
            if(value.item.type==='function_call'){check(!calls.has(value.item.call_id)&&calls.size<128,'Duplicate Responses call id');calls.add(value.item.call_id);}
            items.push({initial:value.item,arguments:value.item.arguments??'',parts:new Map(),summary:new Map()});
            if(value.item.type==='function_call')onProgress({phase:'tool',name:value.item.name,characters:value.item.arguments.length});
            else if(value.item.type==='reasoning')onProgress({phase:'reasoning',characters:0});
            return;
        }
        const item=items[value.output_index];check(item&&!item.done,'Responses item order mismatch');
        if(type==='response.output_item.done'){
            validateItem(value.item,true);check(value.item.id===item.initial.id&&value.item.type===item.initial.type);
            if(value.item.type==='function_call')check(value.item.call_id===item.initial.call_id&&value.item.name===item.initial.name&&value.item.arguments===item.arguments,'Responses tool arguments mismatch');
            if(value.item.type==='message')for(const [index,part]of item.parts)check(value.item.content[index]?.type===part.type&&(value.item.content[index]?.text??value.item.content[index]?.refusal)===part.text,'Responses text mismatch');
            if(value.item.type==='reasoning')for(const [index,part]of item.summary)check(value.item.summary[index]?.text===part.text,'Responses summary mismatch');
            item.done=value.item;return;
        }
        check(value.item_id===item.initial.id,'Responses item id mismatch');
        if(type==='response.function_call_arguments.delta'||type==='response.function_call_arguments.done'){
            check(item.initial.type==='function_call');
            if(type.endsWith('.delta')){check(typeof value.delta==='string');item.arguments+=value.delta;onProgress({phase:'tool',name:item.initial.name,characters:item.arguments.length});}
            else check(value.arguments===item.arguments);
            return;
        }
        const reasoning=type.startsWith('response.reasoning_summary_');
        if(reasoning||type.startsWith('response.content_part.')||type.startsWith('response.output_text.')||type.startsWith('response.refusal.')){
            check(item.initial.type===(reasoning?'reasoning':'message'));
            const index=reasoning?value.summary_index:value.content_index,map=reasoning?item.summary:item.parts;
            check(Number.isInteger(index)&&index>=0&&index<1000);
            if(type.endsWith('_part.added')){
                check(index===map.size&&['summary_text','output_text','refusal'].includes(value.part?.type));
                const text=value.part.text??value.part.refusal;check(typeof text==='string');
                const part={type:value.part.type,text};map.set(index,part);
                if(reasoning){if(text)onReasoning(text);}
                else {const initial=(visibleParts++?'\n':'')+text;if(initial)onDelta(initial);}
                return;
            }
            const part=map.get(index);check(part,'Missing Responses content part');
            if(type.endsWith('.delta')){
                check(typeof value.delta==='string');part.text+=value.delta;
                if(reasoning)onReasoning(value.delta);else onDelta(value.delta);
                onProgress({phase:reasoning?'reasoning':'text',characters:part.text.length});return;
            }
            if(type==='response.output_text.annotation.added')return;
            if(type.endsWith('_part.done'))check((value.part?.text??value.part?.refusal)===part.text);
            else if(type.endsWith('.done'))check((value.text??value.refusal)===part.text);
            else throw new Error('Unsupported Responses content event');
            return;
        }
        throw new Error('Unsupported Responses stream event');
    }
    function text(value){
        pending+=value;let index;
        while((index=pending.indexOf('\n'))!==-1){
            const line=pending.slice(0,index).replace(/\r$/u,'');pending=pending.slice(index+1);
            if(!line)process();else if(line.startsWith('data:'))data.push(line.slice(5).replace(/^ /u,''));
            else if(line==='data')data.push('');else if(line.startsWith('event:'))eventName=line.slice(6).trim();
        }
    }
    try{
        for(;;){signal?.throwIfAborted();const part=await reader.read();signal?.throwIfAborted();if(part.done)break;
            size+=part.value.byteLength;if(size>maxBytes)throw new Error('Response exceeds byte limit');text(decoder.decode(part.value,{stream:true}));}
        text(decoder.decode());check(!pending&&!data.length&&!eventName&&result,'Incomplete Responses stream');return result;
    }catch(error){await reader.cancel().catch(()=>{});throw error;}
}

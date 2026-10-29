export function sendResponses(res,body){
    const task=body.input.filter(item=>item.role==='user').map(item=>typeof item.content==='string'?item.content:item.content.filter(part=>part.type==='input_text').map(part=>part.text).join('\n')).at(-1)??'';
    const results=body.input.filter(item=>item.type==='function_call_output');
    const id='resp_'+crypto.randomUUID();
    let output;
    if(task==='RESPONSES_MAKE'&&!results.length||['RESPONSES_TRUNCATE','RESPONSES_LENGTH','RESPONSES_BAD_BATCH'].includes(task)){
        output=[{type:'reasoning',id:'rs_'+id,summary:[{type:'summary_text',text:'Plan files.'}],encrypted_content:'fixture-encrypted'},
            {type:'function_call',id:'fc_'+id,call_id:'responses_write',name:'write',arguments:JSON.stringify({path:'responses.txt',content:'native responses\n'}),status:'completed'}];
        if(task==='RESPONSES_BAD_BATCH')output.push({type:'function_call',id:'forbidden',call_id:'forbidden',name:'shell',arguments:'{}',status:'completed'});
    }else output=[{type:'message',id:'msg_'+id,role:'assistant',status:'completed',content:[{type:'output_text',text:results.length?'Responses done.':'Responses connected.',annotations:[]}]}];
    const response={object:'response',id,status:task==='RESPONSES_LENGTH'?'incomplete':'completed',output,usage:{input_tokens:10,output_tokens:20}};
    if(!body.stream){res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify(response));return;}
    res.writeHead(200,{'content-type':'text/event-stream'});
    const send=(type,value={})=>res.write(`event: ${type}\ndata: ${JSON.stringify({type,...value})}\n\n`);
    send('response.created',{response:{...response,output:[],status:'in_progress'}});
    for(const [output_index,item]of output.entries()){
        const initial=item.type==='reasoning'?{...item,summary:[],encrypted_content:null}:item.type==='function_call'?{...item,arguments:'',status:'in_progress'}:{...item,content:[],status:'in_progress'};
        send('response.output_item.added',{output_index,item:initial});
        if(item.type==='reasoning'){
            send('response.reasoning_summary_part.added',{output_index,item_id:item.id,summary_index:0,part:{type:'summary_text',text:''}});
            send('response.reasoning_summary_text.delta',{output_index,item_id:item.id,summary_index:0,delta:item.summary[0].text});
        }else if(item.type==='function_call')send('response.function_call_arguments.delta',{output_index,item_id:item.id,delta:item.arguments});
        else{
            send('response.content_part.added',{output_index,item_id:item.id,content_index:0,part:{type:'output_text',text:'',annotations:[]}});
            send('response.output_text.delta',{output_index,item_id:item.id,content_index:0,delta:item.content[0].text});
        }
        send('response.output_item.done',{output_index,item});
    }
    send('response.metadata',{response_id:id,metadata:{fixture:true}});
    if(task!=='RESPONSES_TRUNCATE')send('response.'+response.status,{response});
    res.end();
}

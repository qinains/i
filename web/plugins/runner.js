const FRAME=16*1024*1024,OUTPUT=1024*1024;
function startWorker(){return new Worker(new URL('./worker.js',import.meta.url),{type:'module',name:'harness-wasm-plugin'});}
function errorText(value){return typeof value==='string'?value.slice(0,4096):'Plugin execution failed.';}
function session({signal,timeoutMs=15000,onMessage,message}){
    const worker=startWorker(),token=crypto.randomUUID();let finished=false,paused=false,remaining=timeoutMs,last=performance.now(),timer;
    let resolve,reject;const result=new Promise((a,b)=>{resolve=a;reject=b;});
    function end(error,value){if(finished)return;finished=true;clearTimeout(timer);signal?.removeEventListener('abort',aborted);worker.terminate();error?reject(error):resolve(value);}
    function arm(){last=performance.now();timer=setTimeout(()=>end(new Error('Plugin execution timed out. File changes may already have completed; inspect files before retrying.')),Math.max(0,remaining));}
    function pauseBudget(value){if(finished||paused===value)return;if(value){remaining-=performance.now()-last;clearTimeout(timer);}paused=value;if(!paused)arm();}
    const aborted=()=>end(new Error('Plugin cancelled. File modification outcomes may be unknown; inspect files before retrying.'));
    worker.onerror=event=>{event.preventDefault();end(new Error('Plugin worker failed. File modification outcomes may be unknown.'));};
    worker.onmessage=async event=>{
        if(finished)return;
        try{
            const data=event.data;if(!data||data.token!==token)throw new Error('Invalid plugin worker message.');
            if(data.type==='error'){end(new Error(errorText(data.error)));return;}
            await onMessage(data,{send:body=>{if(!finished)worker.postMessage({token,...body});},end,active:()=>!finished});
        }catch(error){end(error instanceof Error?error:new Error('Plugin protocol failed.'));}
    };
    if(signal?.aborted)aborted();else{signal?.addEventListener('abort',aborted,{once:true});arm();worker.postMessage({token,...message});}
    return {result,pauseBudget,cancel:aborted};
}
export function inspectPluginPackage(bytes,{signal}={}){
    if(!(bytes instanceof Uint8Array)||bytes.length>10*1024*1024)return Promise.reject(new Error('Plugin package exceeds 10 MiB.'));
    return session({signal,message:{type:'inspect',bytes:bytes.slice()},onMessage:(data,{end})=>{if(data.type!=='verified')throw new Error('Invalid plugin inspection result.');end(null,data.verified);}}).result;
}
export function createPluginRun({descriptor,bytes,request,signal,onHostCall,timeoutMs=15000}){
    let waiting=false,count=0;
    return session({signal,timeoutMs,message:{type:'run',descriptor,bytes:bytes.slice(),request},onMessage:async(data,control)=>{
        if(data.identity!==`${descriptor.pluginId}\0${descriptor.version}\0${descriptor.digest}`)throw new Error('Plugin version mismatch.');
        if(data.type==='done'){
            if(waiting||typeof data.output!=='string'||new TextEncoder().encode(data.output).length>OUTPUT)throw new Error('Invalid or oversized plugin output.');
            control.end(null,{output:data.output});return;
        }
        if(data.type!=='host_call'||waiting||++count>128||data.request?.id!==count)throw new Error('Invalid plugin host request sequence.');
        if(new TextEncoder().encode(JSON.stringify(data.request)).length>FRAME)throw new Error('Plugin request exceeds 16 MiB.');
        waiting=true;let response;
        try{response=await onHostCall(data.request);}catch{response={id:data.request.id,ok:false,error:'Host operation failed. File changes may already have completed.'};}
        if(!control.active())return;
        if(response?.id!==data.request.id||typeof response.ok!=='boolean')throw new Error('Invalid host operation response.');
        waiting=false;control.send({type:'resume',response});
    }});
}

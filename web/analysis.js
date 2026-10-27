function invalidSpec(){return new Error('Invalid analysis specification.');}
export function validateAnalysisSpec(spec){
    if(!spec||typeof spec!=='object'||Array.isArray(spec)||Object.keys(spec).sort().join(',')!=='prompt,selection'||typeof spec.prompt!=='string'||!spec.prompt.trim()||[...spec.prompt].length>32000)throw invalidSpec();
    const selection=spec.selection;
    if(!selection||typeof selection!=='object'||Array.isArray(selection)||Object.keys(selection).sort().join(',')!=='end,max_bytes,max_messages,roles,start'||!Array.isArray(selection.roles)||!selection.roles.length||new Set(selection.roles).size!==selection.roles.length||selection.roles.some(role=>!['user','assistant','tool'].includes(role))||!Number.isSafeInteger(selection.start)||selection.start<0||(selection.end!==null&&(!Number.isSafeInteger(selection.end)||selection.end<selection.start))||!Number.isSafeInteger(selection.max_messages)||selection.max_messages<1||selection.max_messages>1000||!Number.isSafeInteger(selection.max_bytes)||selection.max_bytes<1024||selection.max_bytes>262144)throw invalidSpec();
    return {prompt:spec.prompt,selection:{roles:[...selection.roles],start:selection.start,end:selection.end,max_messages:selection.max_messages,max_bytes:selection.max_bytes}};
}
export function narrowAnalysisSpec(spec,scope='all'){
    const request=validateAnalysisSpec(spec);
    if(scope==='all')return request;
    if(!request.selection.roles.includes(scope))throw invalidSpec();
    request.selection.roles=[scope];
    return request;
}
export async function runAnalysis({agent,spec,signal}){
    const request=validateAnalysisSpec(spec);
    signal?.throwIfAborted();
    const cancel=()=>agent.cancel();signal?.addEventListener('abort',cancel,{once:true});
    try{
        const result=await agent.analyze(JSON.stringify(request));
        signal?.throwIfAborted();
        if(typeof result!=='string'||!result.trim()||new TextEncoder().encode(result).length>128000)throw new Error('Analysis returned an invalid result.');
        return result;
    }finally{signal?.removeEventListener('abort',cancel);}
}

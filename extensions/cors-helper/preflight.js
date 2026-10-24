import {makeRule,permissionOrigins} from './rules.js';

export function preflightResponse(config,request){
    if(request.method!=='OPTIONS'||request.url!==config.endpoint)return null;
    const headers=Object.fromEntries(Object.entries(request.headers??{}).map(([key,value])=>[key.toLowerCase(),value]));
    if(headers.origin!==config.origin||headers['access-control-request-method']!=='POST'||headers['access-control-request-private-network'])return null;
    const requested=headers['access-control-request-headers']??'';
    if(typeof requested!=='string'||requested.length>4096||/[\r\n]/u.test(requested)||requested.split(',').some(name=>name.trim()&&!/^[a-z0-9_-]+$/iu.test(name.trim())))return null;
    return {responseCode:204,responseHeaders:[
        {name:'Access-Control-Allow-Origin',value:config.origin},
        {name:'Access-Control-Allow-Methods',value:'POST'},
        {name:'Access-Control-Allow-Headers',value:requested},
        {name:'Access-Control-Max-Age',value:'0'},
    ],body:''};
}

export function createPreflightController(api){
    const sessions=new Map(),queues=new Map();
    const serial=(id,work)=>{const next=(queues.get(id)??Promise.resolve()).catch(()=>{}).then(work);queues.set(id,next);next.finally(()=>{if(queues.get(id)===next)queues.delete(id);}).catch(()=>{});return next;};
    async function stop(id){
        if(!sessions.has(id))return;
        try{await api.debugger.detach({tabId:id});}catch(error){
            // A user-closed debugger is already detached. Other failures remain visible.
            if(sessions.has(id)&&error.message!==`Debugger is not attached to the tab with id: ${id}.`)throw error;
        }
        sessions.delete(id);
    }
    async function enable(id,endpoint,expectedOrigin){
        const tab=await api.tabs.get(id),origin=new URL(tab.url).origin;
        makeRule(id,tab.url,endpoint);
        if(origin!==expectedOrigin||/[?*\\]/u.test(endpoint))throw new Error('页面或 API 地址已改变，请重新打开扩展。');
        if(!await api.permissions.contains({origins:permissionOrigins(tab.url,endpoint)}))throw new Error('请先授权并启用普通 CORS 模式。');
        const rule=makeRule(id,tab.url,endpoint),rules=await api.declarativeNetRequest.getSessionRules();
        if(!rules.some(item=>item.id===rule.id&&item.condition.regexFilter===rule.condition.regexFilter&&item.action.responseHeaders.some(header=>header.header==='access-control-allow-origin'&&header.value===origin)))throw new Error('请先为此地址点击“启用”，再开启预检修复。');
        await stop(id);
        await api.debugger.attach({tabId:id},'1.3');
        const config={endpoint,origin,active:false};sessions.set(id,config);
        try{
            if(new URL((await api.tabs.get(id)).url).origin!==origin)throw new Error('页面已切换。');
            // Chrome does not accept Preflight as a Fetch filter. Exact URL first,
            // then method/origin checks; ordinary requests are immediately resumed.
            await api.debugger.sendCommand({tabId:id},'Fetch.enable',{patterns:[{urlPattern:endpoint,requestStage:'Request'}]});
            if(sessions.get(id)!==config||new URL((await api.tabs.get(id)).url).origin!==origin)throw new Error('调试会话或页面已改变。');
            config.active=true;return {active:true,endpoint};
        }catch(error){await stop(id);error.stage='intercept';throw error;}
    }
    api.debugger.onDetach.addListener(source=>{sessions.delete(source.tabId);});
    api.debugger.onEvent.addListener((source,method,event)=>{
        if(method!=='Fetch.requestPaused')return;
        const target={tabId:source.tabId},config=sessions.get(source.tabId);
        (async()=>{
            let response=null;
            if(config?.active&&event.request.method==='OPTIONS'&&event.request.url===config.endpoint&&new URL((await api.tabs.get(source.tabId)).url).origin===config.origin&&sessions.get(source.tabId)===config)response=preflightResponse(config,event.request);
            await api.debugger.sendCommand(target,response?'Fetch.fulfillRequest':'Fetch.continueRequest',response?{requestId:event.requestId,...response}:{requestId:event.requestId});
        })().catch(async()=>{
            await api.debugger.sendCommand(target,'Fetch.continueRequest',{requestId:event.requestId}).catch(()=>{});
            await serial(source.tabId,()=>sessions.get(source.tabId)===config?stop(source.tabId):undefined).catch(()=>{});
        });
    });
    api.tabs.onRemoved.addListener(id=>{serial(id,()=>stop(id)).catch(()=>{});});
    api.tabs.onUpdated.addListener((id,change)=>{const config=sessions.get(id);if(change.url&&config){let same=false;try{same=new URL(change.url).origin===config.origin;}catch{}if(!same)serial(id,()=>stop(id)).catch(()=>{});}});
    return {
        enable:(id,endpoint,origin)=>serial(id,()=>enable(id,endpoint,origin)),
        disable:id=>serial(id,()=>stop(id)),
        status:id=>{const config=sessions.get(id);return {active:!!config?.active,endpoint:config?.active?config.endpoint:undefined};},
    };
}

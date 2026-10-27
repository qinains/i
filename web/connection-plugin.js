function invalidConnection(){return new Error('Local helper connection is invalid.');}
function helperBase(value,protocol=''){
    // Check the original spelling: URL normalizes integer, hexadecimal and short IP aliases.
    const match=typeof value==='string'&&value.match(/^http:\/\/127\.0\.0\.1:([0-9]{1,5})(\/v1\/(?:responses|messages)|\/)?$/u);
    const expected=protocol==='responses'?'/v1/responses':protocol==='anthropic'?'/v1/messages':null;
    if(!match||Number(match[1])<1||Number(match[1])>65535||(protocol&&match[2]!==expected)||(!protocol&&match[2]&&match[2]!=='/'))throw invalidConnection();
    return `http://127.0.0.1:${match[1]}`;
}
function validateToken(token){if(typeof token!=='string'||!/^[A-Za-z0-9_-]{32,256}$/u.test(token))throw invalidConnection();}
export function validateConnectionPlugin(config){
    if(!['responses','anthropic'].includes(config.protocol)||config.stream!==true||(config.custom_headers??'').trim())throw invalidConnection();
    const base=helperBase(config.base_url,config.protocol);validateToken(config.api_key);
    return base;
}
export function consumePairingFragment(location,history){
    if(!location.hash.startsWith('#connection-plugin='))return null;
    const encoded=location.hash.slice('#connection-plugin='.length);
    history.replaceState(null,'',location.pathname+location.search);
    try{
        if(!/^[A-Za-z0-9_-]+$/u.test(encoded)||encoded.length>2048)throw new Error();
        const pair=JSON.parse(atob(encoded.replace(/-/gu,'+').replace(/_/gu,'/')));
        const url=helperBase(pair.url);validateToken(pair.token);
        return {url,token:pair.token};
    }catch{throw new Error('Pairing link is invalid. Start the local helper again.');}
}
export function connectionPluginDraft(defaults,pair){
    return {...defaults,base_url:helperBase(pair.url)+'/v1/responses',api_key:pair.token,model:'',protocol:'responses',transport:'plugin',stream:true,relay_token:'',custom_headers:''};
}
export function configurationForManifest(config,value,{draft=false}={}){
    const manifest=validateConnectionManifest(value),base=validateConnectionPlugin(config);
    if(config.protocol!==manifest.protocol&&!draft)throw invalidConnection();
    return {...config,protocol:manifest.protocol,base_url:base+(manifest.protocol==='anthropic'?'/v1/messages':'/v1/responses')};
}
export async function localHelperRequest(config,route,{body,signal}={}){
    if(!['manifest','status','login','select','logout','models','cancel'].includes(route))throw invalidConnection();
    const base=validateConnectionPlugin(config);
    const headers={'X-I-Local-Token':config.api_key,accept:'application/json'};
    if(body!==undefined)headers['content-type']='application/json';
    const timeout=AbortSignal.timeout(route==='login'?600000:30000);
    const response=await fetch(`${base}/v1/${route}`,{method:['login','select','logout','cancel'].includes(route)?'POST':'GET',headers,body:body===undefined?undefined:JSON.stringify(body),signal:signal?AbortSignal.any([signal,timeout]):timeout,credentials:'omit',redirect:'error'});
    // Never surface native error bodies, which may contain provider or OAuth data.
    if(!response.ok)throw new Error('Local helper request failed.');
    const text=await response.text();if(text.length>1024*1024)throw new Error('Local helper request failed.');
    return JSON.parse(text);
}
export function validateConnectionManifest(value){
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).sort().join(',')!=='format_version,id,kind,name,protocol'||value.format_version!==1||value.kind!=='connection'||!['responses','anthropic'].includes(value.protocol)||typeof value.id!=='string'||!/^[a-z0-9][a-z0-9.-]{0,127}$/u.test(value.id)||typeof value.name!=='string'||!value.name.trim()||value.name.length>128||/[\x00-\x1f\x7f]/u.test(value.name))throw new Error('Local connection plugin manifest is invalid.');
    return {format_version:1,kind:'connection',id:value.id,name:value.name,protocol:value.protocol};
}
export function visiblePluginModels(value){
    return (Array.isArray(value?.models)?value.models:[]).filter(model=>model?.visibility==='list'&&typeof model.slug==='string'&&model.slug.length>0&&model.slug.length<256&&typeof model.display_name==='string'&&model.display_name.length<=256).map(({slug,display_name,visibility})=>({slug,display_name,visibility}));
}
export function loginAccount(status,selected){
    if(selected==='__new__')return undefined;
    const accounts=(Array.isArray(status?.accounts)?status.accounts:[]).filter(account=>typeof account?.id==='string'&&account.id.length>0&&account.id.length<=256);
    return accounts.find(account=>account.id===selected)?.id??accounts.find(account=>account.id===status?.active)?.id??accounts[0]?.id;
}

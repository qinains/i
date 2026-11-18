export function normalizeEndpoint(value,protocol='openai') {
    if(!['openai','anthropic'].includes(protocol)) throw new Error('Unsupported model protocol.');
    const url=new URL(value);
    if(url.username || url.password || url.search || url.hash || !['https:','http:'].includes(url.protocol)
        || (url.protocol==='http:' && !['localhost','127.0.0.1','[::1]'].includes(url.hostname))) throw new Error('Use HTTPS without URL credentials, query or fragment.');
    let path=url.pathname.replace(/\/+$/u,'');
    if(protocol==='anthropic') {
        if(!path.endsWith('/v1/messages')) path+=path.endsWith('/v1')?'/messages':'/v1/messages';
    } else if(!path.endsWith('/chat/completions')) path+='/chat/completions';
    url.pathname=path;
    return url.href;
}
export function validateCustomHeaders(raw='') {
    const value=raw.trim() ? JSON.parse(raw) : {};
    if(!value || typeof value!=='object' || Array.isArray(value)) throw new Error('Custom headers must be a JSON object.');
    const result={};
    for(const [name,text] of Object.entries(value)) {
        const key=name.toLowerCase();
        if(typeof text!=='string' || !/^[a-z0-9-]+$/u.test(key) || /[\r\n]/u.test(text)
            || ['authorization','proxy-authorization','host','cookie','origin','content-type','accept','content-length','connection','x-api-key','anthropic-version','anthropic-dangerous-direct-browser-access'].includes(key)
            || key.startsWith('x-harness-') || key.startsWith('sec-') || key.startsWith('proxy-')) throw new Error('Invalid or reserved custom header.');
        result[key]=text;
    }
    return result;
}
export async function requestCompletion({endpoint,body,apiKey='',protocol='openai',transport='direct',relayToken='',customHeaders='',controller,onEvent}) {
    const {fetchModel}=await import('./platform.js');
    return fetchModel(normalizeEndpoint(endpoint,protocol),JSON.stringify(body),apiKey,controller ?? new AbortController(),{protocol,transport,relayToken,customHeaders},onEvent);
}

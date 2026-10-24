function checkedUrl(value){
    const url=new URL(value);
    const loopback=['localhost','127.0.0.1','[::1]'].includes(url.hostname);
    if(url.username||url.password||url.hash||!(url.protocol==='https:'||url.protocol==='http:'&&loopback))throw new Error('Use HTTPS, or HTTP on localhost. Credentials and fragments are not allowed.');
    return url;
}
export function permissionOrigins(page,endpoint){
    return [...new Set([page,endpoint].map(value=>{const url=checkedUrl(value);return `${url.protocol}//${url.hostname}/*`;}))];
}
export function makeRule(tabId,page,endpoint){
    if(!Number.isSafeInteger(tabId)||tabId<0)throw new Error('Open the Harness page in a normal tab first.');
    const origin=checkedUrl(page).origin,url=checkedUrl(endpoint);if(url.search)throw new Error('API URL must not contain a query or key.');const target=url.href;
    if(target.length>1500)throw new Error('API URL is too long.');
    return {id:tabId+1,priority:1,action:{type:'modifyHeaders',responseHeaders:[
        {header:'access-control-allow-origin',operation:'set',value:origin},
        {header:'access-control-allow-methods',operation:'set',value:'POST, OPTIONS'},
        {header:'access-control-allow-headers',operation:'set',value:'*, authorization, content-type, x-api-key, anthropic-version, anthropic-dangerous-direct-browser-access'},
        {header:'access-control-max-age',operation:'set',value:'0'},
    ]},condition:{tabIds:[tabId],regexFilter:`^${target.replace(/[.*+?^${}()|[\]\\]/gu,'\\$&')}$`,resourceTypes:['xmlhttprequest']}};
}

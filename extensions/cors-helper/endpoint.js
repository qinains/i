import {makeRule} from './rules.js';

// Serialized into the active page; intentionally reads no form fields or storage.
export function readPublicConnection(){
    const root=document.documentElement;
    const endpoint=root?.getAttribute('data-model-api-url')??'',transport=root?.getAttribute('data-model-transport');
    return {endpoint:endpoint.slice(0,1501),transport:transport==='relay'?'relay':transport==='direct'?'direct':''};
}
export function validateEndpoint(tabId,page,value){
    try{
        const endpoint=new URL(value.trim());if(endpoint.search)throw new Error();
        makeRule(tabId,page,endpoint.href);return endpoint.href;
    }catch{throw new Error('请先在工作台保存 API 设置，或填写有效的完整 API 请求地址。不要填写密钥。');}
}

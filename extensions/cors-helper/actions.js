import {makeRule,permissionOrigins} from './rules.js';
export const api=globalThis.browser??globalThis.chrome;
export async function enableForTab(tabId,endpoint,{expectedOrigin}={}){
    const tab=await api.tabs.get(tabId);
    if(expectedOrigin&&new URL(tab.url).origin!==expectedOrigin)throw new Error('页面已切换，请重新打开扩展后再启用。');
    const rule=makeRule(tabId,tab.url,endpoint);
    if(!await api.permissions.contains({origins:permissionOrigins(tab.url,endpoint)}))throw new Error('Website permission was not granted.');
    const supported=await api.declarativeNetRequest.isRegexSupported({regex:rule.condition.regexFilter,isCaseSensitive:true});
    if(!supported.isSupported)throw new Error('The browser cannot match this API URL safely.');
    rule.condition.isUrlFilterCaseSensitive=true;
    await api.declarativeNetRequest.updateSessionRules({removeRuleIds:[rule.id],addRules:[rule]});
    const origin=new URL(tab.url).origin;
    const assertCurrent=async()=>{
        const current=await api.tabs.get(tabId);
        if(new URL(current.url).origin!==origin)throw new Error('页面已切换，请重新打开扩展后再启用。');
    };
    try{
        await assertCurrent();
        await api.storage.local.set({[`tab-${tabId}`]:{endpoint,origin}});
        await api.action.setBadgeText({tabId,text:'ON'});
        await assertCurrent();
        const rules=await api.declarativeNetRequest.getSessionRules();
        if(!rules.some(item=>item.id===rule.id))throw new Error('授权已失效，请重新打开扩展后再启用。');
    }catch(error){
        await disableForTab(tabId);
        await api.storage.local.remove(`tab-${tabId}`);
        throw error;
    }
    return rule;
}
export async function disableForTab(tabId){
    await api.declarativeNetRequest.updateSessionRules({removeRuleIds:[tabId+1]});
    await api.action.setBadgeText({tabId,text:''}).catch(()=>{});
}

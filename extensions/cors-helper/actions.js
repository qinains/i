import {makeRule,permissionOrigins} from './rules.js';
export const api=globalThis.browser??globalThis.chrome;
export async function enableForTab(tabId,endpoint){
    const tab=await api.tabs.get(tabId),rule=makeRule(tabId,tab.url,endpoint);
    if(!await api.permissions.contains({origins:permissionOrigins(tab.url,endpoint)}))throw new Error('Website permission was not granted.');
    const supported=await api.declarativeNetRequest.isRegexSupported({regex:rule.condition.regexFilter,isCaseSensitive:true});
    if(!supported.isSupported)throw new Error('The browser cannot match this API URL safely.');
    rule.condition.isUrlFilterCaseSensitive=true;
    await api.declarativeNetRequest.updateSessionRules({removeRuleIds:[rule.id],addRules:[rule]});
    await api.storage.local.set({[`tab-${tabId}`]:{endpoint}});
    await api.action.setBadgeText({tabId,text:'ON'});
    return rule;
}
export async function disableForTab(tabId){
    await api.declarativeNetRequest.updateSessionRules({removeRuleIds:[tabId+1]});
    await api.action.setBadgeText({tabId,text:''}).catch(()=>{});
}

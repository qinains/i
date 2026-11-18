const api=globalThis.browser??globalThis.chrome;
async function clearTab(tabId,url){
    const rules=await api.declarativeNetRequest.getSessionRules(),rule=rules.find(item=>item.id===tabId+1);
    if(!rule)return;
    if(url){try{if(new URL(url).origin===rule.action.responseHeaders.find(item=>item.header==='access-control-allow-origin').value)return;}catch{}}
    await api.declarativeNetRequest.updateSessionRules({removeRuleIds:[rule.id]});
    await api.action.setBadgeText({tabId,text:''}).catch(()=>{});
}
api.tabs.onRemoved.addListener(tabId=>{clearTab(tabId).catch(()=>{});api.storage.local.remove(`tab-${tabId}`).catch(()=>{});});
api.tabs.onUpdated.addListener((tabId,change)=>{if(change.url)clearTab(tabId,change.url).catch(()=>{});});

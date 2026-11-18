import {api,enableForTab,disableForTab} from './actions.js';
import {permissionOrigins,makeRule} from './rules.js';
const $=id=>document.getElementById(id);let tab;
function status(text){$('status').textContent=text;}
function busy(value){$('enable').disabled=value;$('disable').disabled=value;}
async function main(){
    [tab]=await api.tabs.query({active:true,currentWindow:true});
    if(!tab?.url)throw new Error('先打开 Harness 网页，再点击扩展图标。');
    makeRule(tab.id,tab.url,'https://api.example/chat/completions');
    $('page').textContent=`当前页面：${new URL(tab.url).origin}`;
    const saved=await api.storage.local.get(`tab-${tab.id}`);$('endpoint').value=saved[`tab-${tab.id}`]?.endpoint??'';
    const rules=await api.declarativeNetRequest.getSessionRules();status(rules.some(rule=>rule.id===tab.id+1)?'已启用 / Enabled':'未启用 / Disabled');busy(false);
}
$('enable').addEventListener('click',async()=>{
    busy(true);
    try{
        const endpoint=$('endpoint').value.trim();makeRule(tab.id,tab.url,endpoint);
        // Keep the permission prompt directly in the user gesture.
        if(!await api.permissions.request({origins:permissionOrigins(tab.url,endpoint)}))throw new Error('未授予网站权限，未启用。');
        await enableForTab(tab.id,endpoint);status('已启用。回到网页重试；不会自动重发请求。');
    }catch(error){
        const rules=await api.declarativeNetRequest.getSessionRules().catch(()=>[]);
        status(`${rules.some(rule=>rule.id===tab?.id+1)?'更改失败，原规则仍启用':'未启用'}：${error.message}`);
    }finally{busy(false);}
});
$('disable').addEventListener('click',async()=>{busy(true);try{await disableForTab(tab.id);status('已关闭 / Disabled');}catch(error){status(error.message);}finally{busy(false);}});
busy(true);main().catch(error=>status(error.message));

import {api,enableForTab,disableForTab} from './actions.js';
import {permissionOrigins,makeRule} from './rules.js';
import {readPublicConnection,validateEndpoint} from './endpoint.js';
const $=id=>document.getElementById(id);let tab,relayMode=false;
const repairSupported=!!api.debugger&&api.runtime.getManifest().permissions.includes('debugger');
function status(text){$('status').textContent=text;}
function busy(value){$('enable').disabled=value||relayMode;$('disable').disabled=value;$('endpoint').disabled=value||relayMode;$('repair-enable').disabled=value||relayMode;$('repair-disable').disabled=value;}
async function repairMessage(action,endpoint){const result=await api.runtime.sendMessage({action,tabId:tab.id,endpoint,origin:new URL(tab.url).origin});if(!result?.ok)throw new Error(result?.error??'预检修复不可用，请重新加载扩展。');return result;}
async function repairStatus(){if(!repairSupported)return;const result=await repairMessage('preflight-status');$('repair-status').textContent=result.active?'已开启，仅修复所选地址的预检；真实 POST 保持原样。':'默认关闭，需要单独确认。';}
async function main(){
    $('repair').hidden=!repairSupported;
    [tab]=await api.tabs.query({active:true,currentWindow:true});
    if(!tab?.url)throw new Error('先打开 Harness 网页，再点击扩展图标。');
    try{makeRule(tab.id,tab.url,'https://api.example/chat/completions');}catch{throw new Error('请先打开正常的工作台网页（HTTPS 或 localhost），再点击扩展图标。');}
    $('page').textContent=`当前页面：${new URL(tab.url).origin}`;
    let connection;try{connection=(await api.scripting.executeScript({target:{tabId:tab.id},func:readPublicConnection}))[0]?.result;}catch{}
    relayMode=connection?.transport==='relay';
    if(relayMode){$('manual').hidden=true;status('网页正在使用中继，无需启用此扩展。');await repairStatus();busy(false);return;}
    let endpoint='';try{if(connection?.endpoint)endpoint=validateEndpoint(tab.id,tab.url,connection.endpoint);}catch{}
    if(endpoint){$('endpoint').value=endpoint;$('detected-url').textContent=endpoint;$('detected-url').hidden=false;$('hint').textContent='已读取生效的 API 地址，不读取密钥。点击启用后按浏览器提示授权。';}
    else{
        const saved=(await api.storage.local.get(`tab-${tab.id}`))[`tab-${tab.id}`];
        try{if(saved?.origin===new URL(tab.url).origin)$('endpoint').value=validateEndpoint(tab.id,tab.url,saved.endpoint);}catch{}
        $('manual').open=true;$('hint').textContent='未自动读取到地址。请先在网页保存 API 设置；旧版页面请更新后重开，也可在下方手动填写。';
    }
    const rules=await api.declarativeNetRequest.getSessionRules(),rule=rules.find(rule=>rule.id===tab.id+1);
    status(rule?endpoint&&rule.condition.regexFilter!==makeRule(tab.id,tab.url,endpoint).condition.regexFilter?'旧地址仍启用，点击启用以更新。':'已启用响应头规则（未验证接口连通）':'未启用');await repairStatus();busy(false);
}
$('enable').addEventListener('click',async()=>{
    busy(true);
    try{
        const endpoint=validateEndpoint(tab.id,tab.url,$('endpoint').value);
        // Keep the permission prompt directly in the user gesture.
        if(!await api.permissions.request({origins:permissionOrigins(tab.url,endpoint)}))throw new Error('未授予网站权限，未启用。');
        await enableForTab(tab.id,endpoint,{expectedOrigin:new URL(tab.url).origin});$('endpoint').value=endpoint;$('detected-url').textContent=endpoint;$('detected-url').hidden=false;status('已启用。仅响应头规则，尚未验证接口连通。回到网页手动重试。');await repairStatus();
    }catch(error){
        const rules=await api.declarativeNetRequest.getSessionRules().catch(()=>[]);
        status(`${rules.some(rule=>rule.id===tab?.id+1)?'更改失败，原规则仍启用':'未启用'}：${error.message}`);
    }finally{busy(false);}
});
$('disable').addEventListener('click',async()=>{busy(true);try{await disableForTab(tab.id);status('已关闭 / Disabled');await repairStatus();}catch(error){status(error.message);}finally{busy(false);}});
$('repair-enable').addEventListener('click',async()=>{
    if(!confirm('为当前标签页和指定 API 开启预检修复？高权限调试器可能接收匹配请求的认证头等元数据，但本实现不记录或保存。仅对 OPTIONS 合成 204，真实 POST、鉴权和回复保持原样。浏览器会显示调试提示；关闭提示或点击“关闭”即可停用。仅用于你信任的页面。'))return;
    busy(true);$('repair-status').textContent='正在开启…';try{const endpoint=validateEndpoint(tab.id,tab.url,$('endpoint').value);await repairMessage('preflight-enable',endpoint);await repairStatus();}catch(error){$('repair-status').textContent=error.message;}finally{busy(false);}
});
$('repair-disable').addEventListener('click',async()=>{busy(true);try{await repairMessage('preflight-disable');await repairStatus();}catch(error){$('repair-status').textContent=error.message;}finally{busy(false);}});
busy(true);main().catch(error=>status(error.message));

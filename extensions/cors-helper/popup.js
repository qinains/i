import {api,enableForTab,disableForTab} from './actions.js';
import {permissionOrigins,makeRule} from './rules.js';
import {readPublicConnection,validateEndpoint} from './endpoint.js';
const $=id=>document.getElementById(id);
let tab,relayMode=false,working=true,enabled=false,repairActive=false,currentRule=null;
const repairSupported=!!api.debugger&&api.runtime.getManifest().permissions.includes('debugger');
function render(){
    $('enable').disabled=working||(relayMode&&!enabled);
    $('enable').textContent=working?'请稍候…':enabled?'停止':'启动';
    $('enable').dataset.active=String(enabled);
    $('endpoint').disabled=working||enabled||relayMode;
    $('status').textContent=relayMode?'无需启动':enabled?repairActive?'已启动 · 预检修复':'已启动 · 普通模式':'未启动';
    $('status').dataset.active=String(enabled);
    let changed=false;
    try{changed=enabled&&currentRule.condition.regexFilter!==makeRule(tab.id,tab.url,$('endpoint').value).condition.regexFilter;}catch{}
    $('next-step').textContent=relayMode?'网页正在使用中继，不需要本扩展。':changed?'网页 API 地址已更改。请先停止，再重新启动。':enabled?repairSupported&&!repairActive?'调试授权已关闭，仅保留普通模式。需要预检修复时，请先停止再启动。':'回到网页发送消息即可。扩展不会自动重发或测试接口。':'点击启动，然后回到网页发送消息。仅作用于当前标签页和下方 API。';
}
async function repairMessage(action,endpoint){const result=await api.runtime.sendMessage({action,tabId:tab.id,endpoint,origin:new URL(tab.url).origin});if(!result?.ok)throw new Error(result?.error??'预检修复不可用，请重新加载扩展。');return result;}
async function refresh(){
    currentRule=(await api.declarativeNetRequest.getSessionRules()).find(item=>item.id===tab.id+1)??null;
    repairActive=repairSupported?(await repairMessage('preflight-status')).active:false;
    enabled=!!currentRule||repairActive;render();
}
function error(value=''){$('error').textContent=value;$('error').hidden=!value;}
async function main(){
    [tab]=await api.tabs.query({active:true,currentWindow:true});
    if(!tab?.url)throw new Error('先打开 Harness 网页，再点击扩展图标。');
    try{makeRule(tab.id,tab.url,'https://api.example/chat/completions');}catch{throw new Error('请先打开正常的工作台网页（HTTPS 或 localhost），再点击扩展图标。');}
    $('page').textContent=new URL(tab.url).origin;
    let connection;try{connection=(await api.scripting.executeScript({target:{tabId:tab.id},func:readPublicConnection}))[0]?.result;}catch{}
    relayMode=connection?.transport==='relay';
    let endpoint='';try{if(connection?.endpoint)endpoint=validateEndpoint(tab.id,tab.url,connection.endpoint);}catch{}
    if(endpoint){$('endpoint').value=endpoint;$('detected-url').textContent=endpoint;$('detected-url').hidden=false;}
    else{
        const saved=(await api.storage.local.get(`tab-${tab.id}`))[`tab-${tab.id}`];
        try{if(saved?.origin===new URL(tab.url).origin)$('endpoint').value=validateEndpoint(tab.id,tab.url,saved.endpoint);}catch{}
        $('manual').open=!relayMode;
    }
    $('manual').hidden=relayMode;
    $('mode-note').textContent=repairSupported?'启动会同时开启响应头规则和预检修复，需确认浏览器调试权限。':'此浏览器仅支持普通模式：无法修复被服务器拒绝的 OPTIONS。';
    await refresh();working=false;render();
}
$('enable').addEventListener('click',async()=>{
    let endpoint;
    try{if(!enabled)endpoint=validateEndpoint(tab.id,tab.url,$('endpoint').value);}catch(failure){error(failure.message);return;}
    if(!enabled&&repairSupported&&!confirm('启动当前标签页到此 API 的 CORS 助手？将同时开启预检修复。高权限调试器可能接收匹配请求的认证头等元数据，本实现不记录或保存。仅为 OPTIONS 合成 204，真实 POST 的鉴权、状态和回复不变。浏览器会显示调试提示；仅用于可信页面。'))return;
    const stopping=enabled;working=true;error();render();
    try{
        if(stopping)await disableForTab(tab.id);
        else{
            // Keep host permission in the click gesture; roll back a partially failed start.
            if(!await api.permissions.request({origins:permissionOrigins(tab.url,endpoint)}))throw new Error('未授予网站权限，未启动。');
            try{
                await enableForTab(tab.id,endpoint,{expectedOrigin:new URL(tab.url).origin});
                if(repairSupported)await repairMessage('preflight-enable',endpoint);
            }catch(failure){
                try{await disableForTab(tab.id);}catch{throw new Error('启动未完成，清理也未完成。请关闭浏览器调试提示，并点击“停止”。');}
                throw failure;
            }
            $('endpoint').value=endpoint;$('detected-url').textContent=endpoint;$('detected-url').hidden=false;$('manual').open=false;
        }
    }catch(failure){error(failure.message);}
    finally{await refresh().catch(failure=>error(failure.message));working=false;render();}
});
window.addEventListener('focus',()=>{if(tab&&!working)refresh().catch(failure=>error(failure.message));});
if(repairSupported)api.debugger.onDetach.addListener(source=>{if(source.tabId===tab?.id&&!working)refresh().catch(()=>{});});
main().catch(failure=>error(failure.message));

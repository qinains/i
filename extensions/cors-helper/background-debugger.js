import './background.js';
import {createPreflightController} from './preflight.js';
const controller=createPreflightController(chrome);
chrome.runtime.onMessage.addListener((message,sender,respond)=>{
    if(sender.id!==chrome.runtime.id||sender.url!==chrome.runtime.getURL('popup.html')||!Number.isSafeInteger(message?.tabId))return;
    let operation;
    if(message.action==='preflight-enable')operation=controller.enable(message.tabId,message.endpoint,message.origin);
    else if(message.action==='preflight-disable')operation=controller.disable(message.tabId);
    else if(message.action==='preflight-status')operation=Promise.resolve(controller.status(message.tabId));
    else return;
    Promise.resolve(operation).then(value=>respond({ok:true,...value}),error=>respond({ok:false,stage:error.stage??'setup',error:'预检修复未完成。请确认普通 CORS 已启用、页面未切换；若打开了开发者工具或其他调试器，请先关闭后重试。'}));
    return true;
});

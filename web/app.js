import {publishArchive} from './download.js';
import {DEFAULT_CONFIGURATION,saveConfiguration,clearConfiguration,deleteProfile,modelMaximum,modelSuggestions} from './config.js';
import {loadConnections,saveConnection} from './connections.js';
import {registerPwa} from './pwa.js';
import {requestCompletion,normalizeEndpoint,validateCustomHeaders} from './transport.js';
import {openStateStore} from './state/store.js';
import {displayName,listWorkspaceDirectories,isTemporaryWorkspace} from './workspaces.js';
import {opfsOpenWorkspace,opfsClearTmp,opfsFileVersion} from './platform.js';
import {FileRefresh} from './file-refresh.js';
import {mountScratchControls} from './scratch-ui.js';
import {mountPreview} from './preview.js';
import {renderFileTree,viewState,persistentView} from './file-browser.js';
import {mountCommands} from './commands.js';
import {updateTitle,compactTitleFromHistory} from './titles.js';
import {TaskController} from './task-controller.js';
import {createBrowserApplication} from './application.js';
import {node,renderMessage} from './render.js';
import {legacyWorkspace} from './state/migration.js';
import {applyWorkspaceTranslations,uiText} from './i18n.js';
import {openPluginStore} from './plugins/store.js';
import {inspectPluginPackage} from './plugins/runner.js';
import {mountPluginSettings} from './plugins/ui.js';
import {mountComposer} from './composer.js';
import {editMessage,removeMessage} from './pending.js';
import {theme} from './theme.js';

const $=id=>document.getElementById(id);
function setText(id,key){const element=$(id);element.dataset.i18n=key;element.textContent=uiText(key);}
let store,controller,workspaces=new Map(),ui={key:'main',openedSessionIds:[],panels:{},scrollPositions:{}},activeId,updateLocked=false,ready=false;
let testController,selectedConnection='',uiQueue=Promise.resolve();
let preview,composerUi;
let pluginUi,pluginBusy=false;
let application;
const fileRefresh=new FileRefresh();
async function ensurePluginManager(){
    if(!application)throw new Error('Application is not ready.');
    return (await application.plugins()).manager;
}
const tasks=new TaskController({
    sessions:()=>controller,blocked:()=>updateLocked||!!testController||pluginBusy||!ready,
    plugins:()=>application.plugins(),online:()=>navigator.onLine,
    connection:()=>({id:selectedConnection,config:agentConfiguration()}),
    changed:()=>{renderTabs();updateControls();},append,report,refresh,redact:text=>redact(text),
    transcriptChanged:record=>{if(record.id===activeId)renderTerminal(true);},
    focus:record=>{if(record.id===activeId)$('input').focus();},
    draftCleared:record=>{if(record.id===activeId){$('input').value='';composerUi.refresh();commandMenu?.hide();}},
    title:(record,title)=>{if(updateTitle(record,'first',redact(title))){controller.later(record);renderTabs();}},
});
let commandCatalog=[],commandMenu;
let viewQueue=Promise.resolve(),listedFiles=[];
const views=new Map();
function fileView(id=active()?.workspaceId){if(!views.has(id))views.set(id,viewState(workspaces.get(id)?.fileView));return views.get(id);}
function saveFileView(id){
    if(isTemporaryWorkspace(id))return Promise.resolve();
    const record=workspaces.get(id);if(!record)return Promise.resolve();
    record.fileView=persistentView(fileView(id));const snapshot=structuredClone(record);
    viewQueue=viewQueue.catch(()=>{}).then(()=>store.commitWorkspace(snapshot));viewQueue.catch(report);return viewQueue;
}
function showFiles(visible){$('file-panel').hidden=!visible;ui.panels.files=visible;$('btn-files').setAttribute('aria-expanded',String(visible));return persistUi();}
function renderFiles(){
    const record=active();if(!record)return;const state=fileView();
    renderFileTree($('file-list'),listedFiles,state,{temporary:isTemporaryWorkspace(record.workspaceId),toggle:(path,open)=>{state.expanded=state.expanded.filter(item=>item!==path);if(open)state.expanded.push(path);saveFileView(record.workspaceId);},select:path=>safe(()=>preview.show(path))()});
    const selected=listedFiles.find(file=>file.path===state.selected&&!file.directory);$('file-details').hidden=!selected;if(selected)$('file-details').textContent=`${selected.path} · ${selected.size} bytes`;
}
let applied={...DEFAULT_CONFIGURATION};
const streams=new Map(),downloads=new Map(),saveStates=new Map();
const active=()=>controller?.records.get(activeId);
const secrets=()=>[applied.api_key,applied.relay_token,$('api-key').value,$('relay-token').value].filter(Boolean);
const redact=text=>secrets().reduce((value,key)=>value.split(key).join('[redacted]'),String(text));
function report(error){$('global-error').hidden=false;$('global-error').textContent=redact(error.message ?? error);}
function safe(fn){return (...args)=>Promise.resolve().then(()=>fn(...args)).catch(report);}
function persistUi(){
    if(!store)return Promise.resolve();
    ui.activeSessionId=activeId;ui.activeWorkspaceId=active()?.workspaceId ?? ui.activeWorkspaceId;
    if(!isTemporaryWorkspace(ui.activeWorkspaceId))ui.lastPersistentSessionId=activeId;
    const snapshot=structuredClone(ui);
    snapshot.openedSessionIds=snapshot.openedSessionIds.filter(id=>!isTemporaryWorkspace(controller.records.get(id)?.workspaceId));
    if(isTemporaryWorkspace(snapshot.activeWorkspaceId)){snapshot.activeWorkspaceId=controller.records.get(ui.lastPersistentSessionId)?.workspaceId??'default';snapshot.activeSessionId=ui.lastPersistentSessionId;}
    uiQueue=uiQueue.catch(()=>{}).then(()=>store.commitUi(snapshot));
    uiQueue.catch(report);return uiQueue;
}
function append(record,text,type='system'){
    if(!record)return;
    record.transcript.push({type,text:redact(text)});controller.later(record);if(record.id===activeId)renderTerminal(true);
}
function renderTerminal(follow=false){
    const record=active();if(!record)return;
    const output=$('output'),previous=output.scrollTop;
    output.replaceChildren(...record.transcript.map(line=>renderMessage(line,{onChange:()=>controller.later(record)})));
    output.scrollTop=follow ? output.scrollHeight : record.scroll ?? previous;
}
function renderTabs(){
    const record=active();if(!record)return;
    $('session-tabs').replaceChildren();
    for(const id of ui.openedSessionIds){
        const item=controller.records.get(id);if(!item || item.workspaceId!==record.workspaceId)continue;
        const group=node('div','','session-tab');group.setAttribute('role','presentation');group.dataset.active=String(id===activeId);
        const tab=node('button','','tab-main');
        tab.append(node('span',`${item.status==='approval'?'● ':tasks.runningId===id?'◌ ':''}${item.name}`,'tab-label'));
        tab.type='button';tab.role='tab';tab.dataset.sessionId=id;tab.setAttribute('aria-selected',String(id===activeId));
        tab.title=`${workspaces.get(item.workspaceId)?.name} · ${item.titleText??item.name}. Double-click to rename.`;
        tab.addEventListener('click',safe(()=>selectSession(id)));
        tab.addEventListener('dblclick',safe(async()=>{if(tasks.runningId===id)return;const name=prompt(uiText('Conversation name:'),item.name);if(name){item.name=displayName(name);item.titleText=item.name;item.titleSource='manual';await controller.save(item);renderTabs();}}));
        const close=node('button','','tab-close');close.type='button';close.setAttribute('aria-label',uiText('Close {name}',{name:item.name}));close.title=close.getAttribute('aria-label');close.disabled=tasks.runningId===id;
        const icon=document.createElementNS('http://www.w3.org/2000/svg','svg');
        for(const [name,value]of Object.entries({width:'12',height:'12',viewBox:'0 0 16 16','aria-hidden':'true'}))icon.setAttribute(name,value);
        const path=document.createElementNS('http://www.w3.org/2000/svg','path');
        for(const [name,value]of Object.entries({d:'M4 4l8 8M12 4l-8 8',fill:'none',stroke:'currentColor','stroke-width':'1.5','stroke-linecap':'round'}))path.setAttribute(name,value);
        icon.append(path);close.append(icon);
        close.addEventListener('click',safe(()=>closeSession(id)));
        group.append(tab,close);$('session-tabs').append(group);
    }
    $('conversation-title').textContent=record.name;
    $('current-path').textContent=`${workspaces.get(record.workspaceId)?.name ?? record.workspaceId}${isTemporaryWorkspace(record.workspaceId)?' [Temporary — reload discards all data]':''} · /workspace`;
    $('workspace-select').value=record.workspaceId;
    $('step-counter').textContent=record.step ?? '';
    updateControls();
}
function updateComposerAction(){
    const running=!!tasks.runningId&&tasks.runningId===activeId;
    const queueDraft=running&&!$('input').disabled&&!!$('input').value.trim();
    $('btn-send').hidden=running&&!queueDraft;
    setText('btn-send',running||active()?.pendingMessages?.length?'Queue message':'Send');
    $('btn-cancel').hidden=!running||queueDraft;$('btn-cancel').disabled=!running;
}
function updateControls(){
    renderStreamStatus();
    const record=active(),blocked=!!tasks.runningId||!!testController||pluginBusy||updateLocked||!ready;
    const preparing=tasks.composerBlocked(activeId);
    $('input').disabled=!record||updateLocked||pluginBusy||!!testController||!ready||preparing;
    for(const id of ['btn-download','btn-upload','file-input','btn-new-session','btn-reset','btn-clear-tmp','btn-download-tmp','upload-target','btn-preview','refresh-preview','preview-scripts','preview-entry','preview-mode']){const el=$(id);if(el)el.disabled=blocked;}
    $('execution-mode').value=record?.mode??'ask';$('execution-mode').disabled=!record||updateLocked||pluginBusy||!!testController||!ready;
    $('mode-control').dataset.mode=record?.mode??'ask';setText('mode-hint',record?.mode==='auto'?'File changes without asking · Shift+Tab':'Shift+Tab to switch');
    $('btn-clear').disabled=!record||updateLocked||tasks.runningId===activeId;
    $('btn-send').disabled=!record||updateLocked||pluginBusy||!!testController||!ready||preparing||!!tasks.runningId&&tasks.runningId!==activeId;
    updateComposerAction();
    setText('status',!ready?'Unavailable':tasks.runningId?'Processing...':'Ready');
    setText('save-status',saveStates.get(activeId) ?? (ready?'Saved':'Opening storage…'));
    $('model-name').textContent=applied.model||uiText('No model');
    $('running-notice').hidden=!tasks.runningId||tasks.runningId===activeId;
    if(tasks.runningId)$('running-notice').querySelector('span').textContent=`${controller.records.get(tasks.runningId)?.name} is running. `;
    const owned=!!tasks.pendingApproval && tasks.pendingApproval.id===activeId;
    $('approval-panel').hidden=!owned;
    if(owned){$('approval-owner').textContent=`${workspaces.get(record.workspaceId)?.name} · ${record.name}`;$('approval-description').textContent=redact(tasks.pendingApproval.description);}
    for(const el of $('settings-modal').querySelectorAll('input,select,textarea,button')){
        if(!['close-settings','cancel-test','theme-select'].includes(el.id) && !el.dataset.settingsTab)el.disabled=blocked;
    }
    pluginUi?.updateControls();
    composerUi?.refresh();
    renderPending();
}
function renderPending(){
    const record=active(),entries=record?.pendingMessages??[];
    $('pending-messages').hidden=!entries.length;
    if(!entries.length){$('queue-list').replaceChildren();return;}
    $('queue-title').textContent=uiText('Pending messages ({count})',{count:entries.length});
    const failed=controller.failed.has(record.id);
    setText('queue-state',failed?'Queue not saved — copy messages before reloading.':record.queuePaused?'Queue paused':'Sends after the current task');
    const blocked=updateLocked||pluginBusy||!!testController||!ready;
    $('queue-resume').hidden=!record.queuePaused;$('queue-pause').hidden=!!record.queuePaused;
    $('queue-resume').disabled=controller.conflicts.has(record.id)||blocked||!!tasks.runningId&&tasks.runningId!==record.id||entries.some(entry=>entry.editDraft!=null);
    $('queue-pause').disabled=blocked;
    const rows=entries.map(entry=>{
        const row=node('div','','pending-message');row.dataset.pendingId=entry.id;
        const sending=tasks.queueSending?.id===record.id&&tasks.queueSending.messageId===entry.id;
        const button=(label,fn)=>{const el=node('button',uiText(label));el.type='button';el.disabled=blocked||sending;el.addEventListener('click',safe(async()=>{try{await fn();await controller.save(record);}finally{updateControls();}}));return el;};
        const actions=node('div','','pending-actions');
        if(entry.editDraft!=null){
            const editor=node('textarea');editor.value=entry.editDraft;editor.setAttribute('aria-label',uiText('Edit queued message'));
            editor.disabled=blocked||sending;
            editor.addEventListener('input',()=>{entry.editDraft=editor.value;controller.later(record);});row.append(editor);
            actions.append(button('Save',()=>editMessage(record,entry.id,entry.editDraft)),button('Discard edits',()=>{delete entry.editDraft;}));
        }else{
            row.append(node('p',redact(entry.visible).slice(0,500)));
            actions.append(button('Edit',()=>{record.queuePaused=true;entry.editDraft=entry.visible;}));
        }
        actions.append(button('Cancel message',()=>removeMessage(record,entry.id)));
        if(sending)actions.append(node('span',uiText('Sending…')));
        row.append(actions);return row;
    });
    $('queue-list').replaceChildren(...rows);
}
function renderStreamStatus(){
    const stream=streams.get(activeId),element=$('stream-status');
    element.hidden=!stream?.requestId;
    if(element.hidden){element.textContent='';return;}
    const count=stream.characters??0;
    const key=stream.phase==='reasoning'?'Model is reasoning… ({count} characters received)':stream.phase==='tool'?'Preparing tool: {name} ({count} characters received; not executed)':stream.phase==='text'?'Receiving response…':'Waiting for model…';
    element.textContent=uiText(key,{count,name:redact((stream.name||'…').slice(0,80))});
}
async function refresh(signal){
    const record=active();if(!record)return;
    return fileRefresh.run(async requestSignal=>{
        const agent=await controller.agent(record);requestSignal.throwIfAborted();
        return agent.list_files(requestSignal);
    },files=>{
    if(record.id!==activeId)return;
    listedFiles=files;renderFiles();
    $('workspace-note').textContent=workspaces.get(record.workspaceId)?.name ?? record.workspaceId;
    $('storage-origin').textContent=uiText(isTemporaryWorkspace(record.workspaceId)?'Site: {origin}. Temporary workspace: all files, including /workspace, are lost on reload.':'Site: {origin}. Workspace: {workspace}. /workspace files remain after reload; only /tmp files are temporary.',{origin:location.origin,workspace:workspaces.get(record.workspaceId)?.name??record.workspaceId});
    $('downloads').replaceChildren(...[...(downloads.get(record.id)?.values() ?? [])].map(artifact=>artifact.link));
    $('download-list').hidden=!downloads.get(record.id)?.size;
    },signal);
}
async function selectSession(id){
    const next=controller.records.get(id);if(!next)throw new Error('Conversation not found.');
    const previous=active();if(previous){previous.draft=$('input').value;previous.scroll=$('output').scrollTop;await controller.flush(previous.id);}
    activeId=id;if(!ui.openedSessionIds.includes(id))ui.openedSessionIds.push(id);
    preview?.selectWorkspace(next.workspaceId);
    $('input').value=next.draft;composerUi?.refresh();renderTerminal();renderTabs();await persistUi();await refresh();updateControls();
    if(previous?.workspaceId!==next.workspaceId)await preview.restore(fileView(next.workspaceId));
}
async function newSession(workspaceId=active()?.workspaceId ?? 'default'){
    const record=await controller.create(workspaceId,uiText('New conversation'),selectedConnection);
    await selectSession(record.id);return record;
}
async function closeSession(id){
    if(tasks.runningId===id)throw new Error('Stop the running task before closing its tab.');
    await controller.flush(id);ui.openedSessionIds=ui.openedSessionIds.filter(item=>item!==id);
    if(activeId===id){const old=active();const next=ui.openedSessionIds.find(key=>controller.records.get(key)?.workspaceId===old.workspaceId);if(next)await selectSession(next);else await newSession(old.workspaceId);}
    renderTabs();await persistUi();
}
function publish(id,archive){
    const record=controller.records.get(id);let list=downloads.get(id);if(!list){list=new Map();downloads.set(id,list);}
    list.get(archive.filename)?.dispose();
    const host=id===activeId?$('downloads'):document.createElement('div');const artifact=publishArchive(archive,host);
    list.set(archive.filename,artifact);artifact.link.textContent=`${workspaces.get(record.workspaceId)?.name} · ${record.name} · ${archive.filename}`;
    append(record,`ZIP ready: ${archive.filename}. Use its save link.`,'success');
    if(id===activeId){$('file-panel').hidden=false;ui.panels.files=true;persistUi();}
    if(id===activeId){$('download-list').hidden=false;$('download-list').open=true;}
    return artifact;
}
function onEvent(id,event){
    const record=controller.records.get(id);if(!record)return;
    if(event.type==='stream_start'){streams.set(id,{requestId:event.request_id,text:'',line:null});if(id===activeId)renderStreamStatus();return;}
    if(event.type==='stream_progress'){
        const stream=streams.get(id);if(stream?.requestId!==event.request_id)return;
        stream.phase=event.phase;stream.name=event.name;stream.characters=event.characters;
        if(id===activeId)renderStreamStatus();return;
    }
    if(['assistant_delta','reasoning_delta','stream_end','stream_error'].includes(event.type)){
        const stream=streams.get(id);if(stream?.requestId!==event.request_id)return;
        if(event.type==='reasoning_delta'){
            if(!stream.reasoningLine){stream.reasoningLine={type:'reasoning',text:'',pending:true,expanded:false};record.transcript.push(stream.reasoningLine);}
            stream.reasoning=(stream.reasoning??'')+event.text;stream.reasoningLine.text=redact(stream.reasoning);
        }else if(event.type==='assistant_delta'){
            if(!stream.line){stream.line={type:'assistant',text:'',pending:true};record.transcript.push(stream.line);}
            stream.text+=event.text;stream.line.text=redact(stream.text);
        }else{
            if(stream.line){delete stream.line.pending;if(event.type==='stream_error')stream.line.text+=' [incomplete]';}
            if(stream.reasoningLine){delete stream.reasoningLine.pending;if(event.type==='stream_error')stream.reasoningLine.text+=' [incomplete]';}
            stream.requestId=null;if(event.type==='stream_error')stream.text=null;
        }
        controller.later(record);if(id===activeId){renderTerminal(true);renderStreamStatus();}return;
    }
    if(event.type==='assistant'&&streams.get(id)?.text===event.text){streams.delete(id);return;}
    if(event.type==='step'){record.step=`[step ${event.step}]`;if(id===activeId)$('step-counter').textContent=record.step;return;}
    if(event.type==='cancelled'){tasks.cancel(id,{cancelAgent:false});return;}
    if(event.type==='tool'){append(record,`tool: ${event.name} ${event.arguments.slice(0,1000)}`,'tool');return;}
    append(record,event.text ?? '',event.type==='assistant'?'assistant':'tool');
}
async function submit(text){
    const record=active();if(!record||!text.trim())return;
    if(await tasks.submit(record,text))return;
    text=text.trim();record.draft='';$('input').value='';append(record,`> ${text}`,'user');
    commandMenu?.hide();
    if(text.startsWith('!')){append(record,'Shell commands are disabled in browser mode.','error');return;}
    if(text==='/help'){append(record,commandCatalog.filter(command=>command.platforms.includes('web')).map(command=>`${command.name}${command.arguments.length?' '+command.arguments.join('|'):''} — ${uiText(command.description)}`).join('\n'));return;}
    if(/^\/mode(?:\s|$)/u.test(text)){const value=text.split(/\s+/u);if(value.length===1||value[1]==='status'&&value.length===2)append(record,uiText('Mode: {mode}',{mode:record.mode??'ask'}));else if(value.length===2&&['ask','auto'].includes(value[1]))await changeMode(value[1]);else append(record,'Usage: /mode [ask|auto|status]','error');return;}
    if(text==='/clear'){record.transcript=[];renderTerminal();await controller.save(record);return;}
    if(text==='/new'){await newSession();return;}
    if(text==='/files'){await refresh();await showFiles(true);return;}
    if(text==='/download'){await tasks.run(record,async(agent,signal)=>{const zip=await agent.export_zip('{}',signal);signal.throwIfAborted();publish(record.id,zip);},{readOnly:true});return;}
    if(text.startsWith('/download ')){await tasks.run(record,async(agent,signal)=>{const zip=await agent.export_zip(JSON.stringify({paths:[text.slice(10).trim()]}),signal);signal.throwIfAborted();publish(record.id,zip);},{readOnly:true});return;}
    if(text==='/resume'){showHistory();return;}
    if(text.startsWith('/resume ')){const value=text.slice(8).trim(),matches=[...controller.records.values()].filter(item=>item.workspaceId===record.workspaceId&&(item.id===value||item.name===value));if(matches.length!==1)throw new Error(uiText('Choose a unique conversation from /resume.'));await selectSession(matches[0].id);return;}
    if(text==='/config'){openSettings();return;}
    if(text.startsWith('/')){append(record,'Unknown command. Use /help.','error');return;}
}
async function changeMode(mode){
    if(!['ask','auto'].includes(mode)||updateLocked||testController)throw new Error('Cannot change execution mode now.');
    const record=active();if(!record)return;record.mode=mode;await controller.save(record);updateControls();append(record,uiText('Mode: {mode}',{mode}));
}
async function upload(files){
    const record=active();if(!record)return;
    await tasks.run(record,async(agent,signal)=>{
        for(const file of files){
            signal.throwIfAborted();
            if(file.size>8*1024*1024)throw new Error(`${file.name} exceeds 8 MiB`);
            const path=$('upload-target').value+(file.webkitRelativePath||file.name);
            const expected=await opfsFileVersion(await opfsOpenWorkspace(record.workspaceId),path,signal);
            signal.throwIfAborted();
            if(expected!==null&&!await tasks.approve(record.id,`Overwrite uploaded file ${path}?`)){append(record,`Skipped ${path}`);continue;}
            const bytes=new Uint8Array(await file.arrayBuffer());signal.throwIfAborted();
            await agent.upload(path,bytes,signal,expected);append(record,`Uploaded: ${path} (${file.size} bytes)`,'success');
        }
    });
}
function currentConfiguration(){
    return {base_url:$('base-url').value.trim(),model:$('model').value.trim(),api_key:$('api-key').value,protocol:$('protocol').value,max_tokens:Number($('max-tokens').value),reasoning_effort:$('reasoning-effort').value,use_max_tokens:$('use-max-tokens').checked,language:$('lang-select').value,transport:$('transport').value,stream:$('stream').checked,relay_token:$('relay-token').value,custom_headers:$('custom-headers').value.trim()};
}
function agentConfiguration(){const {language,...config}=applied;return config;}
function restoreConfiguration(config){
    for(const [id,key]of [['base-url','base_url'],['model','model'],['api-key','api_key'],['protocol','protocol'],['relay-token','relay_token'],['custom-headers','custom_headers'],['reasoning-effort','reasoning_effort'],['max-tokens','max_tokens'],['transport','transport'],['lang-select','language']])$(id).value=config[key] ?? DEFAULT_CONFIGURATION[key];
    $('stream').checked=config.stream;$('use-max-tokens').checked=config.use_max_tokens;updateTransport();applyWorkspaceTranslations(config.language);updateEndpoint();
}
function updateTransport(){
    const relay=$('transport').value==='relay';
    $('relay-options').hidden=!relay;
    $('connection-extra').hidden=relay;$('cors-help').hidden=relay;
}
function updateEndpoint(){
    const anthropic=$('protocol').value==='anthropic';
    $('base-url').placeholder=anthropic?'https://api.anthropic.com':'https://api.deepseek.com';
    setText('endpoint-help',anthropic?'Enter the API root URL, /v1, or full /v1/messages URL.':'Enter the API root URL or full /chat/completions URL. The endpoint is appended only once.');
    const maximum=modelMaximum(currentConfiguration());
    const suggestions=modelSuggestions(currentConfiguration());
    $('model-options').replaceChildren(...suggestions.map(name=>{const option=node('option');option.value=name;return option;}));
    setText('model-help',suggestions.length?'Reference models from provider documentation, not your account permissions. Select a suggestion or enter a model ID manually.':'Enter the model ID supplied by your API provider. No verified model catalog or thinking capabilities are available for this endpoint.');
    $('use-model-max').hidden=!maximum;$('reasoning-options').hidden=!maximum;
    if(maximum){$('max-tokens').max=maximum;$('use-max-tokens').checked=true;}else $('max-tokens').removeAttribute('max');
    $('token-parameter-option').hidden=anthropic||!!maximum;$('anthropic-tokens-note').hidden=!anthropic;
    try{$('resolved-endpoint').textContent=uiText('Request URL: {url}',{url:normalizeEndpoint($('base-url').value.trim(),$('protocol').value)});}catch{$('resolved-endpoint').textContent=uiText('The request URL appears after a valid API URL is entered.');}
}
function validateConfig(config){normalizeEndpoint(config.base_url,config.protocol);validateCustomHeaders(config.custom_headers);if(!config.model.trim()||!Number.isSafeInteger(config.max_tokens)||config.max_tokens<1)throw new Error('Model and a positive output token limit are required.');const maximum=modelMaximum(config);if(maximum&&config.max_tokens>maximum)throw new Error('DeepSeek maximum output tokens: 393216.');if(config.transport==='relay'&&!config.relay_token)throw new Error('Relay access token is required.');}
function updateProfiles(){
    const connections=loadConnections(localStorage);let names={};try{names=JSON.parse(localStorage.getItem('i_harness_connection_names')??'{}');}catch{}
    const memory=node('option',uiText('This session only'));memory.dataset.i18n='This session only';$('profile-select').replaceChildren(memory);
    $('profile-select').firstChild.value='';
    for(const item of connections.records){const option=node('option',names[item.id] ?? item.name);option.value=item.id;$('profile-select').append(option);}
    $('profile-select').value=selectedConnection;
}
function saveConfig(){
    if(tasks.runningId||testController||updateLocked)throw new Error('Finish the active request first.');
    const config=currentConfiguration();validateConfig(config);
    if((config.api_key!==applied.api_key||config.custom_headers!==applied.custom_headers) && [...controller.records.values()].some(record=>record.connectionId===selectedConnection&&record.history.some(message=>message.role==='user')) && !confirm(uiText('Changing this account or tenant may send existing conversation history to a different recipient. Apply this credential change?')))return;
    if($('remember-config').checked){
        if(!selectedConnection)selectedConnection='default';
        saveConnection(localStorage,selectedConnection,config);updateProfiles();
        setText('config-status','Configuration saved in this browser.');
    }else setText('config-status','Applied for this session only.');
    applied={...config};$('model-name').textContent=config.model;applyWorkspaceTranslations(config.language);updateEndpoint();updateControls();
}
function openSettings(){if(!$('settings-modal').open)$('settings-modal').showModal();updateControls();if(!document.querySelector('[data-settings-panel=plugins]').hidden)pluginUi?.refresh();}
function showHistory(){
    const record=active();$('history-list').replaceChildren();
    for(const item of [...controller.records.values()].filter(item=>item.workspaceId===record.workspaceId).sort((a,b)=>b.updatedAt-a.updatedAt)){
        const row=node('button',`${item.name} · ${new Date(item.updatedAt).toLocaleString()} · ${item.status}`);
        row.addEventListener('click',safe(async()=>{await selectSession(item.id);$('history-modal').close();}));$('history-list').append(row);
    }
    $('history-modal').showModal();
}
function bind(){
    theme.subscribe(mode=>{$('theme-select').value=mode;});
    $('theme-select').addEventListener('change',()=>{const saved=theme.setMode($('theme-select').value);setText('config-status',saved?'Theme saved locally; connection changes still require Save.':'Theme applied for this page; browser storage is unavailable.');});
    composerUi=mountComposer({input:$('input'),list:$('composer-attachments'),getRecord:active,isDisabled:()=>tasks.composerBlocked(activeId)||!!testController||pluginBusy||updateLocked||!ready,onError:report,onChange:(record,value)=>{record.draft=value;controller.later(record);commandMenu?.hide();updateComposerAction();}});
    pluginUi=mountPluginSettings({manager:ensurePluginManager,currentWorkspace:()=>active()?.workspaceId,isBusy:()=>!!tasks.runningId||!!testController||updateLocked||!ready,onError:report,onBusy:value=>{pluginBusy=value;updateControls();}});
    preview=mountPreview({owner:active,isBusy:()=>!!tasks.runningId||!!testController||updateLocked,onError:report,onChange:patch=>{const record=active();if(!record)return;Object.assign(fileView(),patch);saveFileView(record.workspaceId);renderFiles();if(patch.open&&matchMedia('(max-width:700px)').matches)showFiles(false);},onBack:()=>showFiles(true)});
    mountScratchControls({
        create:safe(async()=>{if(tasks.runningId||updateLocked)return;const name=prompt(uiText('Temporary workspace name (all files are lost on reload; download first):'));if(!name)return;const id=`temp-${crypto.randomUUID()}`;await opfsOpenWorkspace(id);workspaces.set(id,{id,name:displayName(name),storageKind:'temporary'});renderWorkspaces();await newSession(id);}),
        clear:safe(async()=>{if(!confirm('Clear /tmp in this workspace? Normal workspace files and conversations are retained.'))return;const record=active();await tasks.run(record,async(_agent,signal)=>{await opfsClearTmp(record.workspaceId,signal);append(record,'Cleared /tmp. Workspace files retained.');});}),
        download:safe(()=>{const record=active();return tasks.run(record,async(agent,signal)=>{const zip=await agent.export_zip('{"paths":["/tmp"],"filename":"tmp.zip"}',signal);signal.throwIfAborted();publish(record.id,zip).link.click();},{readOnly:true});}),
    });
    const attach=node('button','Attach selected connection to this conversation…');attach.id='attach-connection';attach.dataset.i18n='Attach selected connection to this conversation…';
    $('settings').append(attach);
    attach.addEventListener('click',safe(async()=>{
        const record=active();if(!record||tasks.runningId)return;
        validateConfig(applied);
        if(!confirm(uiText('Attach the selected connection? Future requests will send this conversation history to its API account and tenant. No request is sent now.')))return;
        record.connectionId=selectedConnection;record.connectionIdentity='';await controller.save(record);
        setText('config-status','Connection attached. No request was sent.');
    }));
    $('input-line').addEventListener('submit',event=>{event.preventDefault();if(!event.isComposing)safe(()=>submit($('input').value))();});
    $('input').addEventListener('keydown',event=>{if(event.key==='Enter'&&(event.isComposing||event.keyCode===229))event.preventDefault();});
    $('input').addEventListener('keydown',event=>{if(event.key==='Tab'&&event.shiftKey&&!event.isComposing&&!event.repeat&&$('command-menu')?.hidden){event.preventDefault();if($('execution-mode').disabled)return;safe(async()=>{const mode=active().mode==='auto'?'ask':'auto';if(mode==='auto'&&!confirm(uiText('Automatically approve file changes in this conversation? Shell remains disabled. Pending approvals still require your decision.')))return;await changeMode(mode);})();}});
    $('input').addEventListener('input',()=>{const record=active();if(record){record.draft=$('input').value;controller.later(record);}updateComposerAction();});
    $('output').addEventListener('scroll',()=>{const record=active();if(record){record.scroll=$('output').scrollTop;controller.later(record);}});
    $('btn-new-session').addEventListener('click',safe(()=>newSession()));
    $('btn-clear').addEventListener('click',safe(async()=>{if(tasks.runningId===activeId)return;const record=active();record.transcript=[];await controller.save(record);renderTerminal();}));
    $('btn-reset').addEventListener('click',safe(async()=>{if(tasks.runningId)return;if(!confirm('Reset this conversation context? Workspace files are retained.'))return;const record=active(),agent=await controller.agent(record);agent.new_session();record.history=JSON.parse(agent.export_history());record.connectionIdentity='';record.journal=null;append(record,'Conversation context reset. Workspace files retained.');await controller.save(record);}));
    $('btn-cancel').addEventListener('click',()=>{if(tasks.runningId===activeId){fileRefresh.cancel();tasks.cancel(activeId,{pause:true});}});
    $('queue-pause').addEventListener('click',safe(()=>tasks.pause(active())));
    $('queue-resume').addEventListener('click',safe(()=>tasks.resume(active())));
    $('jump-running').addEventListener('click',safe(()=>selectSession(tasks.runningId)));
    $('approve').addEventListener('click',()=>tasks.settleApproval(true,activeId));$('deny').addEventListener('click',()=>tasks.settleApproval(false));
    $('btn-download').addEventListener('click',safe(()=>{const record=active();return tasks.run(record,async(agent,signal)=>{const zip=await agent.export_zip('{}',signal);signal.throwIfAborted();publish(record.id,zip).link.click();},{readOnly:true});}));
    $('btn-upload').addEventListener('click',()=>$('file-input').click());
    $('file-input').addEventListener('change',safe(async()=>{const files=[...$('file-input').files];$('file-input').value='';await upload(files);}));
    $('btn-files').addEventListener('click',safe(()=>showFiles($('file-panel').hidden)));
    $('close-files').addEventListener('click',safe(()=>showFiles(false)));
    $('execution-mode').addEventListener('change',safe(async()=>{const mode=$('execution-mode').value;if(mode==='auto'&&!confirm(uiText('Automatically approve file changes in this conversation? Shell remains disabled. Pending approvals still require your decision.'))){updateControls();return;}await changeMode(mode);}));
    $('output').addEventListener('dragover',event=>event.preventDefault());$('output').addEventListener('drop',event=>{event.preventDefault();if(!tasks.runningId)safe(()=>upload([...event.dataTransfer.files]))();});
    $('workspace-select').addEventListener('change',safe(async()=>{const value=$('workspace-select').value;if(value==='__new__'){const name=prompt(uiText('New workspace name:'));if(!name)return;$('workspace-select').value='';const record={id:crypto.randomUUID(),name:displayName(name),createdAt:Date.now(),storageKind:'persistent'};await opfsOpenWorkspace(record.id);await store.createWorkspace(record);workspaces.set(record.id,record);renderWorkspaces();await newSession(record.id);return;}if(value==='__scratch__'){$('workspace-select').value='';if(tasks.runningId||updateLocked)return;const name=prompt(uiText('Temporary workspace name (all files are lost on reload; download first):'));if(!name)return;const id=`temp-${crypto.randomUUID()}`;await opfsOpenWorkspace(id);workspaces.set(id,{id,name:displayName(name),storageKind:'temporary'});renderWorkspaces();await newSession(id);return;}const found=ui.openedSessionIds.map(id=>controller.records.get(id)).find(record=>record?.workspaceId===value)??[...controller.records.values()].find(record=>record.workspaceId===value);if(found)await selectSession(found.id);else await newSession(value);}));
    $('btn-history').addEventListener('click',showHistory);$('close-history').addEventListener('click',()=>$('history-modal').close());
    $('settings-btn').addEventListener('click',openSettings);$('close-settings').addEventListener('click',()=>$('settings-modal').close());
    $('settings-modal').addEventListener('close',()=>{const focused=document.activeElement;if(focused===document.body||$('settings-modal').contains(focused))$('settings-btn').focus();});
    for(const button of document.querySelectorAll('[data-settings-tab]'))button.addEventListener('click',()=>{for(const panel of document.querySelectorAll('[data-settings-panel]'))panel.hidden=panel.dataset.settingsPanel!==button.dataset.settingsTab;for(const item of document.querySelectorAll('[data-settings-tab]'))item.setAttribute('aria-selected',String(item===button));document.querySelector('#settings-modal .dialog-footer').hidden=button.dataset.settingsTab==='plugins';if(button.dataset.settingsTab==='plugins')pluginUi.refresh();});
    $('transport').addEventListener('change',updateTransport);
    $('toggle-key').addEventListener('click',()=>{$('api-key').type=$('api-key').type==='password'?'text':'password';setText('toggle-key',$('api-key').type==='password'?'Show':'Hide');});
    $('settings-modal').addEventListener('input',()=>{setText('config-status','Unsaved changes');});
    $('base-url').addEventListener('input',updateEndpoint);$('model').addEventListener('input',updateEndpoint);$('protocol').addEventListener('change',updateEndpoint);
    $('use-model-max').addEventListener('click',()=>{const maximum=modelMaximum(currentConfiguration());if(maximum){$('max-tokens').value=maximum;$('use-max-tokens').checked=true;setText('config-status','Unsaved changes');}});
    $('lang-select').addEventListener('change',safe(async()=>{
        applied.language=$('lang-select').value;applyWorkspaceTranslations(applied.language);localStorage.setItem('i_lang',applied.language);updateControls();updateEndpoint();await refresh();setText('config-status','Language updated; connection changes still require Save.');
    }));
    $('save-config').addEventListener('click',safe(saveConfig));
    $('remember-config').addEventListener('change',safe(()=>{if($('remember-config').checked)saveConfig();else{clearConfiguration(localStorage);selectedConnection='';updateProfiles();setText('config-status','Saved credentials removed; current values are memory-only.');}}));
    $('save-as-profile').addEventListener('click',safe(()=>{const name=prompt(uiText('Connection name:'));if(!name)return;const config=currentConfiguration();validateConfig(config);selectedConnection=crypto.randomUUID();let names={};try{names=JSON.parse(localStorage.getItem('i_harness_connection_names')??'{}');}catch{}names[selectedConnection]=displayName(name);localStorage.setItem('i_harness_connection_names',JSON.stringify(names));$('remember-config').checked=true;saveConfig();updateProfiles();}));
    $('profile-select').addEventListener('change',safe(()=>{const found=loadConnections(localStorage).records.find(item=>item.id===$('profile-select').value);selectedConnection=found?.id??'';if(found){restoreConfiguration(found.settings);applied={...found.settings};$('remember-config').checked=true;$('model-name').textContent=applied.model;setText('config-status','Saved connection selected. Existing conversations keep their context.');updateControls();}}));
    $('delete-profile').addEventListener('click',safe(()=>{if(!selectedConnection||!confirm(uiText('Delete this saved connection? Files and conversation history are retained.')))return;deleteProfile(localStorage,selectedConnection);selectedConnection='';updateProfiles();setText('config-status','Connection deleted; current values remain in memory.');}));
    $('clear-config').addEventListener('click',safe(()=>{if(tasks.runningId||!confirm(uiText('Clear ALL saved model and relay credentials? Files and conversations will remain.')))return;clearConfiguration(localStorage);for(const agent of controller.agents.values())agent.forget_credentials();applied={...DEFAULT_CONFIGURATION,language:applied.language};restoreConfiguration(applied);selectedConnection='';$('remember-config').checked=false;updateProfiles();$('model-name').textContent=uiText('No model');setText('config-status','Saved and active credentials cleared; files and conversations retained.');}));
    $('btn-clear-history').addEventListener('click',safe(async()=>{if(tasks.runningId||!confirm(uiText('Delete ALL saved conversations on this device? Workspace files and credentials are retained.')))return;const reports=await Promise.all([...new Set([...controller.records.keys(),...controller.agents.keys(),...controller.agentStarts.keys()])].map(id=>controller.close(id)));if(reports.some(report=>!report.closed||report.errors.length))throw new Error('Conversation cleanup failed; saved conversations were retained.');for(const id of controller.records.keys())await store.deleteSession(id);controller.records.clear();ui.openedSessionIds=[];activeId=null;await newSession(ui.activeWorkspaceId??'default');}));
    $('test-connection').addEventListener('click',safe(async()=>{
        if(tasks.runningId||testController)return;const config=currentConfiguration();validateConfig(config);testController=new AbortController();$('cancel-test').hidden=false;updateControls();setText('test-result','Testing…');
        const body={model:config.model,messages:[{role:'user',content:'Hi'}],stream:false,[config.protocol==='anthropic'||config.use_max_tokens?'max_tokens':'max_completion_tokens']:5};
        if(config.protocol==='anthropic'){body.system=[];body.messages[0].content=[{type:'text',text:'Hi'}];}
        try{const result=JSON.parse(await requestCompletion({endpoint:config.base_url,protocol:config.protocol,body,apiKey:config.api_key,transport:config.transport,relayToken:config.relay_token,customHeaders:config.custom_headers,controller:testController}));if(config.protocol==='anthropic'?result.type!=='message'||result.role!=='assistant'||!Array.isArray(result.content):!result.choices?.[0]?.message)throw new Error(uiText('Unexpected response; model connection not verified.'));setText('test-result','Connection verified.');}
        catch(error){delete $('test-result').dataset.i18n;$('test-result').textContent=redact(error.message??error);}finally{testController=null;$('cancel-test').hidden=true;updateControls();}
    }));
    $('cancel-test').addEventListener('click',()=>testController?.abort());
    window.addEventListener('pagehide',event=>{tasks.cancel(tasks.runningId);controller?.cancelAll();testController?.abort();for(const map of downloads.values())for(const artifact of map.values())artifact.dispose();if(!event.persisted)application?.requestShutdown();});
}
function renderWorkspaces(){
    const options=[...workspaces.values()].map(record=>{const option=node('option',record.name+(isTemporaryWorkspace(record.id)?' [Temporary]':''));option.value=record.id;return option;});
    const divider=node('option','────────────────');divider.disabled=true;options.push(divider);
    const newOption=node('option','+ New Workspace…');newOption.value='__new__';options.push(newOption);
    const scratchOption=node('option',' Scratch Workspace…');scratchOption.value='__scratch__';options.push(scratchOption);
    $('workspace-select').replaceChildren(...options);
}
async function main(){
    bind();restoreConfiguration(DEFAULT_CONFIGURATION);
    try{
        await opfsOpenWorkspace('default');store=await openStateStore();const saved=await store.load();workspaces=new Map(saved.workspaces.map(record=>[record.id,record]));
        if(!workspaces.has('default')){const record={id:'default',name:'Default',createdAt:Date.now(),storageKind:'persistent'};await store.createWorkspace(record);workspaces.set('default',record);}
        if(!saved.ui?.legacyMigration){
            try{const migration=legacyWorkspace(localStorage);if(migration){
                const record={...workspaces.get('default'),name:migration.name};await store.commitWorkspace(record);workspaces.set('default',record);
                saved.ui={...(saved.ui??{}),key:'main',legacyMigration:migration};await store.commitUi(saved.ui);
                report(new Error('Legacy workspace labels shared one file directory. Its files remain in the default workspace; create new workspaces for real isolation. Original labels were retained.'));
            }}catch{report(new Error('Legacy workspace labels could not be migrated; original data and files were retained.'));}
        }
        for(const id of await listWorkspaceDirectories())if(!workspaces.has(id)){
            if(confirm(`Found an unlisted workspace (${id.slice(0,8)}). Restore it without changing its files?`)){const record={id,name:`Recovered ${id.slice(0,8)}`,createdAt:Date.now(),storageKind:'persistent'};await store.createWorkspace(record);workspaces.set(id,record);}
        }
        const response=await fetch(new URL('./commands.json',import.meta.url),{credentials:'omit'});if(!response.ok)throw new Error('Cannot load command catalog.');commandCatalog=await response.json();
        commandMenu=mountCommands($('input'),commandCatalog,()=>({files:listedFiles,sessions:[...controller.records.values()].filter(item=>item.workspaceId===active()?.workspaceId)}));
        const {default:init,BrowserAgent}=await import('./pkg/i.js');await init();
        application=createBrowserApplication({Agent:BrowserAgent,store,openPluginStore,inspectPackage:inspectPluginPackage,hooks:{event:onEvent,approve:(id,description)=>tasks.approve(id,description),download:publish,error:report,committed:record=>{if(updateTitle(record,'compact',redact(compactTitleFromHistory(record.history))))renderTabs();},saving:(id,state)=>{saveStates.set(id,state);if(id===activeId)setText('save-status',state);}}});
        controller=application.controller;
        const connections=loadConnections(localStorage);for(const warning of connections.warnings)report(new Error(warning));
        const selected=connections.records.find(record=>record.id===connections.current);if(selected){selectedConnection=selected.id;applied={...selected.settings};restoreConfiguration(applied);$('remember-config').checked=true;$('model-name').textContent=applied.model;}
        await controller.restore(saved.sessions.filter(record=>workspaces.has(record.workspaceId)),(text,record)=>{
            const bound=connections.records.find(connection=>connection.id===record.connectionId)?.settings;
            return [bound?.api_key,bound?.relay_token].filter(Boolean).reduce((text,key)=>text.split(key).join('[redacted]'),redact(text));
        });
        let language;try{language=localStorage.getItem('i_lang');}catch{}if([...$('lang-select').options].some(option=>option.value===language)){applied.language=language;$('lang-select').value=language;applyWorkspaceTranslations(language);updateEndpoint();}
        updateProfiles();if(saved.ui)ui={...ui,...saved.ui,panels:{...ui.panels,...saved.ui.panels}};
        ui.openedSessionIds=(ui.openedSessionIds??[]).filter(id=>controller.records.has(id));renderWorkspaces();
        const chosen=controller.records.get(ui.activeSessionId)??[...controller.records.values()][0];
        if(chosen){await selectSession(chosen.id);}else{await newSession('default');append(active(),'i — browser workspace. Shell is disabled; file tools use persistent OPFS.\nConversations save automatically. Use /help for commands.');}
        await controller.flush();ready=true;
        $('file-panel').hidden=!ui.panels.files;$('btn-files').setAttribute('aria-expanded',String(!!ui.panels.files));updateControls();
        registerPwa({isBusy:()=>!!tasks.runningId||!!testController||pluginBusy,onLock:value=>{updateLocked=value;updateControls();},onStatus:text=>{$('pwa-status').textContent=text;},beforeUpdate:async()=>{await controller.flush();await uiQueue;await viewQueue;}}).catch(error=>report(new Error(`PWA unavailable: ${error.message}`)));
    }catch(error){ready=false;updateControls();report(error);$('status').textContent='Unavailable';$('output').append(node('div',`Initialization failed: ${redact(error.message)}. Build with scripts/build_web.sh and serve over localhost/HTTPS.`,'line error'));}
}
main();

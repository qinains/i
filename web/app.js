import {publishArchive} from './download.js';
import {saveConfiguration,loadConfiguration,clearConfiguration,DEFAULT_CONFIGURATION} from './config.js';
import {registerPwa} from './pwa.js';

const $ = id => document.getElementById(id);
const output = $('output');
const input = $('input');
let agent;
let busy = false;
let updateLocked = false;
let pendingApproval;
let streamBlock;
let streamedText = null;
let streamId;
const artifacts = new Map();
const redact = text => [$('api-key').value,$('relay-token').value].filter(Boolean).reduce((value,key) => value.split(key).join('[redacted]'),String(text));
function line(text, type = 'system') {
    const div = document.createElement('div'); div.className = `line ${type}`; div.textContent = redact(text);
    output.append(div); output.scrollTop = output.scrollHeight;
}
function setBusy(value, cancellable = false) {
    busy = value;
    value = value || updateLocked;
    for (const id of ['input','btn-upload','file-input','btn-download','btn-new','btn-clear']) $(id).disabled = value || !agent;
    $('settings').disabled = value;
    $('lang-select').disabled = value;
    $('btn-cancel').disabled = !value || !cancellable;
    $('status').textContent = value ? 'Processing...' : agent ? 'Ready' : 'Unavailable';
}
function settleApproval(allowed) {
    const resolve = pendingApproval; pendingApproval = null;
    $('approval-panel').hidden = true;
    if (resolve) resolve(allowed);
}
function approve(description) {
    $('approval-description').textContent = redact(description);
    $('approval-panel').hidden = false;
    return new Promise(resolve => {pendingApproval = resolve;});
}
function publish(archive) {
    artifacts.get(archive.filename)?.dispose();
    const artifact = publishArchive(archive, $('downloads'));
    artifacts.set(archive.filename, artifact);
    line(`ZIP ready: ${archive.filename}. Use its save link.`, 'success');
    return artifact;
}
function event(event) {
    if (event.type === 'stream_start') {
        streamId = event.request_id; streamedText = ''; streamBlock = null; return;
    }
    if (['assistant_delta','stream_end','stream_error'].includes(event.type)) {
        if (event.request_id !== streamId) return;
        if (event.type === 'assistant_delta') {
            if (!streamBlock) {streamBlock = document.createElement('div'); streamBlock.className = 'line assistant'; output.append(streamBlock);}
            streamedText += event.text; streamBlock.textContent = redact(streamedText); output.scrollTop = output.scrollHeight;
        } else {
            if (event.type === 'stream_error') {
                if (streamBlock) {streamBlock.classList.add('error'); streamBlock.textContent += ' [incomplete]';}
                streamedText = null;
            }
            streamId = null;
        }
        return;
    }
    if (event.type === 'assistant' && streamedText === event.text) {streamedText = null; return;}
    if (event.type === 'cancelled') { settleApproval(false); return; }
    if (event.type === 'step') { $('step-counter').textContent = `[step ${event.step}/${event.max_steps}]`; return; }
    if (event.type === 'tool') { line(`tool: ${event.name} ${event.arguments.slice(0,1000)}`, 'tool'); return; }
    line(event.text ?? '', event.type === 'assistant' ? 'assistant' : 'tool');
}
async function refresh() {
    const files = await agent.list_files();
    $('file-list').replaceChildren();
    if (!files.length) { $('file-list').textContent = 'No files in workspace'; return; }
    for (const file of files) {
        const row = document.createElement('div'); row.className = 'file-item';
        row.textContent = `${file.path}${file.directory ? '/' : ` (${file.size} bytes)`}`;
        $('file-list').append(row);
    }
}
function currentConfiguration() {
    return {base_url:$('base-url').value.trim(),model:$('model').value.trim(),api_key:$('api-key').value,protocol:'openai',
        max_steps:Number($('max-steps').value),max_tokens:Number($('max-tokens').value),use_max_tokens:$('use-max-tokens').checked,
        language:$('lang-select').value,transport:$('transport').value,stream:$('stream').checked,relay_token:$('relay-token').value};
}
function restoreConfiguration(config) {
    for (const [id,key] of [['base-url','base_url'],['model','model'],['api-key','api_key'],['max-steps','max_steps'],['max-tokens','max_tokens'],['lang-select','language']]) $(id).value = config[key];
    $('use-max-tokens').checked = config.use_max_tokens;
    $('transport').value = config.transport;
    $('stream').checked = config.stream;
    $('relay-token').value = config.relay_token;
    document.documentElement.lang = config.language;
}
function persistConfiguration() {
    if (!$('remember-config').checked) return;
    try {
        saveConfiguration(localStorage,currentConfiguration());
        $('config-status').textContent = 'Configuration saved in this browser.';
    } catch {
        $('config-status').textContent = 'Configuration could not be saved. Check values and browser storage permissions.';
    }
}
function configure() {
    const {language,...config} = currentConfiguration();
    agent.configure(JSON.stringify(config));
    persistConfiguration();
    $('model-name').textContent = `Model: ${$('model').value.trim()}`;
}
async function operation(fn, cancellable = false) {
    if (busy || updateLocked || !agent) return;
    setBusy(true, cancellable);
    try { await fn(); } catch (error) { line(`Error: ${error.message ?? error}`, 'error'); }
    finally {
        settleApproval(false);
        try { await refresh(); } catch (error) { line(`File list: ${error.message ?? error}`, 'error'); }
        setBusy(false); input.focus();
    }
}
async function submit(text) {
    if (busy || updateLocked || !agent || !text) return;
    if (!navigator.onLine && !text.startsWith('/')) {line('Offline: AI needs a network connection. Local files and ZIP export remain available.','error'); return;}
    line(`> ${text}`, 'user');
    if (text.startsWith('!')) { line('Shell commands are disabled in browser mode.', 'error'); return; }
    if (text === '/help') { line('/help /clear /new /files /download. Shell is disabled.'); return; }
    if (text === '/clear') { output.replaceChildren(); return; }
    if (text === '/new') { agent.new_session(); line('New model session; workspace files retained.'); return; }
    if (text === '/files') { await refresh(); return; }
    if (text === '/download') { await operation(async () => publish(await agent.export_zip('{}'))); return; }
    if (text.startsWith('/')) { line('Unknown command. Use /help.', 'error'); return; }
    await operation(async () => { configure(); await agent.run(text); }, true);
}
input.addEventListener('keydown', async event => {
    if (event.key !== 'Enter' || event.isComposing || event.keyCode === 229 || busy || !agent) return;
    event.preventDefault(); const text = input.value.trim(); input.value = ''; await submit(text);
});
$('approve').addEventListener('click', () => settleApproval(true));
$('deny').addEventListener('click', () => settleApproval(false));
$('btn-cancel').addEventListener('click', () => {agent?.cancel(); settleApproval(false);});
$('btn-clear').addEventListener('click', () => output.replaceChildren());
$('btn-new').addEventListener('click', () => {agent.new_session(); line('New model session; workspace files retained.');});
$('btn-download').addEventListener('click', () => operation(async () => {
    const artifact = publish(await agent.export_zip('{}')); artifact.link.click();
}));
$('btn-upload').addEventListener('click', () => $('file-input').click());
$('file-input').addEventListener('change', () => {
    const files = [...$('file-input').files]; $('file-input').value = '';
    return operation(async () => {
        for (const file of files) {
            if (file.size > 8 * 1024 * 1024) throw new Error(`${file.name} exceeds 8 MiB`);
            const path = file.webkitRelativePath || file.name;
            const existing = await agent.list_files();
            if (existing.some(item => item.path === path) && !await approve(`Overwrite uploaded file ${path}?`)) {
                line(`Skipped ${path}`); continue;
            }
            await agent.upload(path,new Uint8Array(await file.arrayBuffer()));
            line(`Uploaded: ${path} (${file.size} bytes)`, 'success');
        }
    });
});
$('lang-select').addEventListener('change', () => {
    try {localStorage.setItem('i_lang',$('lang-select').value);} catch { /* Optional preference storage. */ }
    document.documentElement.lang = $('lang-select').value;
    persistConfiguration();
});
$('settings').addEventListener('input', persistConfiguration);
$('remember-config').addEventListener('change', () => {
    if ($('remember-config').checked) persistConfiguration();
    else {
        try {clearConfiguration(localStorage); $('config-status').textContent = 'Saved configuration removed; current values are memory-only.';}
        catch {$('config-status').textContent = 'Could not remove saved configuration. Clear site storage in browser settings.';}
    }
});
$('clear-config').addEventListener('click', () => {
    try {
        clearConfiguration(localStorage); localStorage.removeItem('i_lang');
        agent?.clear_configuration();
        restoreConfiguration(DEFAULT_CONFIGURATION); $('remember-config').checked = false;
        $('model-name').textContent = 'Model: -';
        $('config-status').textContent = 'Saved and active configuration cleared; workspace files retained.';
    } catch {$('config-status').textContent = 'Could not clear configuration. Clear site storage in browser settings.';}
});
window.addEventListener('pagehide', () => {agent?.cancel(); for (const artifact of artifacts.values()) artifact.dispose();});

async function main() {
    registerPwa({isBusy:() => busy,onLock:value => {updateLocked = value; setBusy(busy);},onStatus:text => {$('pwa-status').textContent = text;}})
        .catch(() => {$('pwa-status').textContent = 'PWA installation unavailable; browser workspace can still be used online.';});
    setBusy(false); $('status').textContent = 'Loading WASM...';
    try {
        const saved = loadConfiguration(localStorage);
        if (saved) {
            restoreConfiguration(saved); $('remember-config').checked = true;
            $('config-status').textContent = 'Saved configuration restored from this browser.';
        } else {
            const language = localStorage.getItem('i_lang');
            if ([...$('lang-select').options].some(option => option.value === language)) $('lang-select').value = language;
        }
    } catch {$('config-status').textContent = 'Saved configuration unavailable or invalid. Clear it to start again.';}
    try {
        const {default:init, BrowserAgent} = await import('./pkg/i.js');
        await init(); agent = await BrowserAgent.create(event, approve, publish);
        setBusy(false); await refresh();
        line('i — browser workspace. Shell is disabled; file tools use persistent OPFS.');
        line('Configure a CORS-enabled Chat Completions endpoint and model. Configuration is memory-only unless Remember is enabled.');
        line('Use /help for commands. Clearing site data deletes this workspace.');
    } catch (error) {
        agent = null; setBusy(false); line(`Initialization failed: ${error.message ?? error}. Build with scripts/build_web.sh and serve over localhost/HTTPS.`, 'error');
    }
}
main();

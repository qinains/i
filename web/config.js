export const CONFIG_KEY = 'i_harness_config';
export const DEFAULT_CONFIGURATION = Object.freeze({base_url:'',model:'',api_key:'',protocol:'openai',
    max_steps:32,max_tokens:8192,use_max_tokens:true,language:'en'});

function validated(settings) {
    if (!settings || typeof settings !== 'object') throw new Error('Invalid harness configuration.');
    const result = {};
    for (const key of ['base_url','model','api_key','protocol','language']) {
        if (typeof settings[key] !== 'string') throw new Error('Invalid harness configuration.');
        result[key] = settings[key];
    }
    if (settings.protocol !== 'openai' || !['en','zh-CN','zh-TW','ja','ko','fr','de'].includes(settings.language)
        || !Number.isInteger(settings.max_steps) || settings.max_steps < 1 || settings.max_steps > 32
        || !Number.isSafeInteger(settings.max_tokens) || settings.max_tokens < 1
        || typeof settings.use_max_tokens !== 'boolean') throw new Error('Invalid harness configuration.');
    return {...result,max_steps:settings.max_steps,max_tokens:settings.max_tokens,use_max_tokens:settings.use_max_tokens};
}
export function saveConfiguration(storage, settings) {
    storage.setItem(CONFIG_KEY,JSON.stringify({version:1,settings:validated(settings)}));
}
export function loadConfiguration(storage) {
    const raw = storage.getItem(CONFIG_KEY);
    if (raw === null) return null;
    try {
        const record = JSON.parse(raw);
        if (record?.version !== 1) throw new Error('Unsupported version');
        return validated(record.settings);
    } catch { throw new Error('Saved harness configuration is invalid or unsupported. Clear it to start again.'); }
}
export function clearConfiguration(storage) { storage.removeItem(CONFIG_KEY); }

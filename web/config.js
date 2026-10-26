import {languageCodes} from './languages.js';
export const CONFIG_KEY = 'i_harness_config';
export const PROFILES_KEY = 'i_harness_profiles';
export const DEFAULT_CONFIGURATION = Object.freeze({base_url:'',model:'',api_key:'',protocol:'openai',
    max_tokens:65536,use_max_tokens:true,reasoning_effort:'default',language:'en',transport:'direct',stream:true,relay_token:'',custom_headers:''});

export function modelSuggestions(settings){
    try{
        const url=new URL(settings.base_url);
        if(settings.protocol==='openai'&&url.protocol==='https:'&&url.hostname==='api.deepseek.com')return ['deepseek-flash','deepseek-v4-pro'];
    }catch{}
    return [];
}
export function modelMaximum(settings){
    return modelSuggestions(settings).includes(settings.model)?393216:null;
}

function validated(settings) {
    if (!settings || typeof settings !== 'object') throw new Error('Invalid harness configuration.');
    settings = {...settings,custom_headers:settings.custom_headers ?? '',reasoning_effort:settings.reasoning_effort ?? 'default'};
    const result = {};
    for (const key of ['base_url','model','api_key','protocol','language','transport','relay_token','custom_headers']) {
        if (typeof settings[key] !== 'string') throw new Error('Invalid harness configuration.');
        result[key] = settings[key];
    }
    if (!['openai','anthropic','responses'].includes(settings.protocol) || !languageCodes.includes(settings.language)
        || !['default','none','low','medium','high','max','xhigh'].includes(settings.reasoning_effort)
        || !Number.isSafeInteger(settings.max_tokens) || settings.max_tokens < 1
        || !['direct','relay'].includes(settings.transport) || typeof settings.stream !== 'boolean'
        || typeof settings.use_max_tokens !== 'boolean') throw new Error('Invalid harness configuration.');
    return {...result,max_tokens:settings.max_tokens,use_max_tokens:settings.use_max_tokens,reasoning_effort:settings.reasoning_effort,stream:settings.stream};
}
export function saveConfiguration(storage, settings) {
    storage.setItem(CONFIG_KEY,JSON.stringify({version:2,settings:validated(settings)}));
}
export function loadConfiguration(storage) {
    const raw = storage.getItem(CONFIG_KEY);
    if (raw === null) return null;
    try {
        const record = JSON.parse(raw);
        if (![1,2].includes(record?.version)) throw new Error('Unsupported version');
        return validated(record.version === 1 ? {...record.settings,transport:'direct',stream:true,relay_token:''} : record.settings);
    } catch { throw new Error('Saved harness configuration is invalid or unsupported. Clear it to start again.'); }
}
export function clearConfiguration(storage) { storage.removeItem(CONFIG_KEY); storage.removeItem(PROFILES_KEY); }

// Profile management
export function loadProfiles(storage) {
    const raw = storage.getItem(PROFILES_KEY);
    if (!raw) return {profiles: {}, current: null};
    try {
        const data = JSON.parse(raw);
        return {profiles: data.profiles || {}, current: data.current || null};
    } catch {
        return {profiles: {}, current: null};
    }
}
export function saveProfiles(storage, profiles, current) {
    storage.setItem(PROFILES_KEY, JSON.stringify({profiles, current}));
}
export function saveProfile(storage, name, settings) {
    const {profiles, current} = loadProfiles(storage);
    profiles[name] = validated(settings);
    saveProfiles(storage, profiles, current);
}
export function loadProfile(storage, name) {
    const {profiles} = loadProfiles(storage);
    return Object.hasOwn(profiles,name) ? validated(profiles[name]) : null;
}
export function deleteProfile(storage, name) {
    const {profiles, current} = loadProfiles(storage);
    delete profiles[name];
    const newCurrent = current === name ? null : current;
    saveProfiles(storage, profiles, newCurrent);
}
export function setCurrentProfile(storage, name) {
    const {profiles, current} = loadProfiles(storage);
    saveProfiles(storage, profiles, name);
}
export function getCurrentProfile(storage) {
    return loadProfiles(storage).current;
}
export function getProfileNames(storage) {
    return Object.keys(loadProfiles(storage).profiles);
}
export function migrateToProfiles(storage) {
    // Migrate legacy single config to default profile
    const legacyConfig = loadConfiguration(storage);
    if (legacyConfig) {
        const {profiles, current} = loadProfiles(storage);
        if (!current && Object.keys(profiles).length === 0) {
            profiles['default'] = legacyConfig;
            saveProfiles(storage, profiles, 'default');
            storage.removeItem(CONFIG_KEY);
        }
    }
}

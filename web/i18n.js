import {copy} from './ui-copy.js';
import {detectLanguage,languageDirection,resolveLanguage} from './languages.js';
export {detectLanguage};
export function hasTranslation(key){return Object.hasOwn(copy.en,key);}

export function t(key,language,values={}){
    const text=copy[language]?.[key]??copy.en[key]??key;
    return text.replace(/\{(\w+)\}/gu,(match,name)=>{
        if(!Object.hasOwn(values,name))return match;
        const value=String(values[name]);
        return languageDirection(language)==='rtl'?`\u2068${value}\u2069`:value;
    });
}
export function uiText(key,values={}){return t(key,document.documentElement.lang,values);}
export function applyTranslations(preference){
    const language=resolveLanguage(preference),root=document.documentElement;
    root.lang=language;root.dir=languageDirection(language);
    for(const element of document.querySelectorAll('[data-i18n]')){
        if(!element.querySelector('*'))element.textContent=t(element.dataset.i18n,language);
    }
    for(const [attribute,key]of [['placeholder','i18nPlaceholder'],['title','i18nTitle'],['aria-label','i18nAriaLabel']]){
        const selector=attribute==='aria-label'?'data-i18n-aria-label':`data-i18n-${attribute}`;
        for(const element of document.querySelectorAll(`[${selector}]`))element.setAttribute(attribute,t(element.dataset[key],language));
    }
}
export function applyWorkspaceTranslations(language){
    applyTranslations(language);
    const groups={connection:'Model connection',recovery:'Workspace & recovery',appearance:'Appearance',advanced:'Advanced',plugins:'Plugins'};
    for(const element of document.querySelectorAll('[data-settings-tab]'))element.textContent=uiText(groups[element.dataset.settingsTab]??element.dataset.settingsTab);
}

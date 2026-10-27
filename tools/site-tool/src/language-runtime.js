
export const languageCodes=languageOptions.map(item=>item.code);
export function detectLanguage(preferences=[],supported=languageCodes){
    for(const preference of preferences??[]){
        if(typeof preference!=='string')continue;
        const tag=preference.replaceAll('_','-').toLowerCase(),parts=tag.split('-');
        const exact=supported.find(code=>code.toLowerCase()===tag);if(exact)return exact;
        const candidate=parts[0]==='zh'
            ? parts.includes('hans')?'zh-CN':parts.includes('hant')||parts.some(part=>['tw','hk','mo'].includes(part))?'zh-TW':'zh-CN'
            : supported.find(code=>code.toLowerCase()===parts[0]);
        if(supported.includes(candidate))return candidate;
    }
    return supported.includes('en')?'en':supported[0]??'en';
}
export function resolveLanguage(saved,preferences=[]){return languageCodes.includes(saved)?saved:detectLanguage(preferences);}
export function languageDirection(code){return languageOptions.find(item=>item.code===code)?.dir??'ltr';}
export function populateLanguageSelect(select){
    const previous=select.value;
    select.replaceChildren(...languageOptions.map(item=>{
        const option=select.ownerDocument.createElement('option');option.value=item.code;option.textContent=item.name;option.lang=item.code;option.dir=item.dir;return option;
    }));
    if(languageCodes.includes(previous))select.value=previous;
}

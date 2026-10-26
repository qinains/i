import {languageCodes,detectLanguage,resolveLanguage} from './languages.js';
import {catalogs} from './translations.js';
export {catalogs};
export const languages=languageCodes;
export const detectSiteLanguage=detectLanguage;
export const resolveSiteLanguage=resolveLanguage;
export function translate(language,key){return catalogs[language]?.[key]??catalogs.en[key]??key;}

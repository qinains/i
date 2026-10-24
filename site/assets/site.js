import {resolveSiteLanguage,translate} from './i18n.js';
const root=document.documentElement;
const home=root.dataset.page==='home';let savedLanguage,language;
try{savedLanguage=localStorage.getItem('lninl_site_language');}catch{}
function applyLanguage(next){
    language=next;root.lang=next;document.getElementById('site-language').value=next;
    for(const element of document.querySelectorAll('[data-site-i18n]'))element.textContent=translate(next,element.dataset.siteI18n);
    for(const element of document.querySelectorAll('[data-site-i18n-aria]'))element.setAttribute('aria-label',translate(next,element.dataset.siteI18nAria));
    for(const element of document.querySelectorAll('[data-site-i18n-placeholder]'))element.placeholder=translate(next,element.dataset.siteI18nPlaceholder);
    if(home){document.title=translate(next,'page.title');document.querySelector('meta[name=description]').content=translate(next,'hero.description');}
    document.dispatchEvent(new CustomEvent('site-language-change',{detail:{language:next}}));
}
applyLanguage(resolveSiteLanguage(savedLanguage,navigator.languages?.length?navigator.languages:[navigator.language]));
document.getElementById('site-language').addEventListener('change',event=>{
    applyLanguage(resolveSiteLanguage(event.target.value,[]));try{localStorage.setItem('lninl_site_language',language);}catch{}
    document.getElementById('search').dispatchEvent(new Event('input'));
});
try{const saved=localStorage.getItem('lninl_site_theme');if(saved==='light'||saved==='dark')root.dataset.theme=saved;}catch{}
document.getElementById('theme').addEventListener('click',()=>{
    const dark=root.dataset.theme?root.dataset.theme==='dark':home||matchMedia('(prefers-color-scheme:dark)').matches;
    root.dataset.theme=dark?'light':'dark';try{localStorage.setItem('lninl_site_theme',root.dataset.theme);}catch{}
});
if(matchMedia('(max-width:720px)').matches)document.querySelector('.docs-menu')?.removeAttribute('open');
for(const link of document.querySelectorAll('.sidebar a'))if(new URL(link.href).pathname===location.pathname)link.setAttribute('aria-current','page');
const input=document.getElementById('search'),results=document.getElementById('search-results');let indexPromise,searchGeneration=0;
input.addEventListener('input',async()=>{
    const generation=++searchGeneration,query=input.value.trim().toLocaleLowerCase();results.replaceChildren();results.hidden=!query;if(!query)return;
    try{
        indexPromise??=fetch('/search-index.json',{credentials:'omit'}).then(response=>{if(!response.ok)throw new Error('Search unavailable');return response.json();});
        const pages=await indexPromise;if(generation!==searchGeneration)return;
        const terms=query.split(/\s+/u),matches=pages.filter(page=>terms.every(term=>(page.title+' '+page.text).toLocaleLowerCase().includes(term))).slice(0,10);
        for(const page of matches){const li=document.createElement('li'),link=document.createElement('a');link.href=page.url;link.textContent=page.title;li.append(link);results.append(li);}
        if(!matches.length){const li=document.createElement('li');li.textContent=translate(language,'search.empty');results.append(li);}
    }catch{if(generation!==searchGeneration)return;indexPromise=null;results.textContent=translate(language,'search.error');}
});
input.addEventListener('keydown',event=>{if(event.key==='Escape'){results.hidden=true;input.blur();}});
document.addEventListener('click',event=>{if(!event.target.closest('.search-bar'))results.hidden=true;});
if(home)import('./home.js');

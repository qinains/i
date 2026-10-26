import {fetchCatalog} from '/app/plugins/catalog.js';
import {translate} from './i18n.js';

const list=document.getElementById('plugins-list');
const status=document.getElementById('plugins-status');
const load=document.getElementById('plugins-load');
let plugins=[],state='',loading=false;
const t=key=>translate(document.documentElement.lang,key);
function element(tag,text){const node=document.createElement(tag);node.textContent=text;return node;}
function externalLink(text,url){
    const link=element('a',text);link.href=url;link.target='_blank';link.rel='noopener noreferrer';link.referrerPolicy='no-referrer';return link;
}
function render(){
    document.title=t('plugins.title')+' · i';
    status.textContent=state?t('plugins.'+state):'';
    list.replaceChildren();
    for(const plugin of plugins){
        const card=document.createElement('article');card.className='plugin-card';
        card.append(element('h2',plugin.name),element('code',plugin.id+' · '+plugin.version),element('p',plugin.description));
        card.append(element('p',t('plugins.permissions')),element('pre',JSON.stringify(plugin.permissions,null,2)));
        const links=document.createElement('nav');
        links.append(externalLink(t('plugins.repository'),plugin.repository),externalLink(t('plugins.download'),plugin.package.url));
        card.append(links);list.append(card);
    }
}
load.addEventListener('click',async()=>{
    if(loading)return;
    loading=true;load.disabled=true;state='loading';render();
    try{const catalog=await fetchCatalog();plugins=catalog.plugins;state=plugins.length?'updated':'empty';}
    catch{plugins=[];state='error';}
    finally{loading=false;load.disabled=false;render();}
});
document.addEventListener('site-language-change',render);
render();
load.click();

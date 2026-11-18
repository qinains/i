const root=document.documentElement;
try{const saved=localStorage.getItem('lninl_site_theme');if(saved==='light'||saved==='dark')root.dataset.theme=saved;}catch{}
document.getElementById('theme').addEventListener('click',()=>{
    const dark=root.dataset.theme?root.dataset.theme==='dark':matchMedia('(prefers-color-scheme:dark)').matches;
    root.dataset.theme=dark?'light':'dark';try{localStorage.setItem('lninl_site_theme',root.dataset.theme);}catch{}
});
if(matchMedia('(max-width:720px)').matches)document.querySelector('.docs-menu')?.removeAttribute('open');
for(const link of document.querySelectorAll('.sidebar a'))if(new URL(link.href).pathname===location.pathname)link.setAttribute('aria-current','page');
const input=document.getElementById('search'),results=document.getElementById('search-results');let indexPromise;
input.addEventListener('input',async()=>{
    const query=input.value.trim().toLocaleLowerCase();results.replaceChildren();results.hidden=!query;if(!query)return;
    try{
        indexPromise??=fetch('/search-index.json',{credentials:'omit'}).then(response=>{if(!response.ok)throw new Error('Search unavailable');return response.json();});
        const pages=await indexPromise;if(input.value.trim().toLocaleLowerCase()!==query)return;
        const terms=query.split(/\s+/u),matches=pages.filter(page=>terms.every(term=>(page.title+' '+page.text).toLocaleLowerCase().includes(term))).slice(0,10);
        for(const page of matches){const li=document.createElement('li'),link=document.createElement('a');link.href=page.url;link.textContent=page.title;li.append(link);results.append(li);}
        if(!matches.length){const li=document.createElement('li');li.textContent='未找到结果，请尝试其他关键词。';results.append(li);}
    }catch{indexPromise=null;results.textContent='搜索暂不可用，可从文档目录浏览。';}
});
input.addEventListener('keydown',event=>{if(event.key==='Escape'){results.hidden=true;input.blur();}});
document.addEventListener('click',event=>{if(!event.target.closest('.search-bar'))results.hidden=true;});

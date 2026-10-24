const tasks={
    website:{files:['▾ src','  ◇ index.html','  ◇ style.css','  ◇ app.js'],code:'glob({patterns:["**/*"]})\nread({paths:["index.html"]})\nwrite({files:[...]})\ndownload({filename:"site.zip"})',extension:'.zip'},
    refactor:{files:['▾ src','  ◇ main.rs','  ◇ config.rs','▸ tests'],code:'search({pattern:"Config",paths:["src"]})\nread({paths:["src/config.rs"]})\nedit({path:"src/config.rs",edits:[...]})',extension:'.rs'},
    plugin:{files:['▾ plugins','  ◇ plugin.json','  ◇ plugin.wasm','◇ README.md'],code:'read({paths:["README.md"]})\n// installed tool, scoped permissions\nword_count({path:"README.md"})\n→ {words:128, lines:24}',extension:'.wasm'}
};
let selected='website';
function update(){
    const task=tasks[selected];
    document.getElementById('demo-prompt').dataset.siteI18n=`demo.prompt.${selected}`;
    document.getElementById('demo-result').dataset.siteI18n=`demo.result.${selected}`;
    for(const [index,file]of task.files.entries())document.querySelector(`[data-demo-file="${index}"]`).textContent=file;
    document.getElementById('demo-code').textContent=task.code;
    document.querySelector('.result-extension').textContent=task.extension;
    for(const button of document.querySelectorAll('[data-demo-task]'))button.setAttribute('aria-pressed',String(button.dataset.demoTask===selected));
}
import {translate} from './i18n.js';
function updateCopy(){
    for(const id of ['demo-prompt','demo-result']){const element=document.getElementById(id);element.textContent=translate(document.documentElement.lang,element.dataset.siteI18n);}
}
for(const button of document.querySelectorAll('[data-demo-task]'))button.addEventListener('click',()=>{selected=button.dataset.demoTask;update();updateCopy();});
document.addEventListener('site-language-change',updateCopy);
update();updateCopy();

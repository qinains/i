import {init,parse} from './module-lexer.js';
const ORIGIN='https://workspace.invalid',LIMIT=48*1024*1024;
function dataUrl(text){
    const bytes=new TextEncoder().encode(text);let binary='';
    for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));
    return 'data:text/javascript;base64,'+btoa(binary);
}
// Stable virtual names in an import map preserve native cycles and module identity.
export async function moduleGraph(files,scripts,entry,translate=key=>key){
    await init;
    const imports=Object.create(null),pending=new Map();let expanded=0;
    const fail=(key,path='')=>{throw new Error(translate(key).replaceAll('{path}',path));};
    const encode=text=>{const url=dataUrl(text);expanded+=url.length;if(expanded>LIMIT)fail('Expanded module preview exceeds 48 MiB.');return url;};
    function resolve(value,base){
        if(typeof value!=='string'||!value)fail('Computed dynamic imports are not supported. Use a literal local path.');
        if(!/^(?:\.\.?\/|\/)/u.test(value))fail(/^[\w+.-]+:/u.test(value)?'External CDN modules are disabled. Use local JavaScript files.':'npm package imports are not supported. Use a local relative path or a bundled build.');
        let url,path;try{url=new URL(value,base);path=decodeURIComponent(url.pathname);}catch{fail('Invalid module path: {path}',value);}
        if(url.origin!==ORIGIN||url.username||url.password||/[\\\0]/u.test(path))fail('External CDN modules are disabled. Use local JavaScript files.');
        if(!Object.hasOwn(files,path))fail('Module file not found: {path}',path);
        if(files[path].type!=='text/javascript')fail('Only JavaScript modules are supported: {path}',path);
        if(!pending.has(url.href)){
            if(pending.size>=1000)fail('Module preview exceeds 1000 modules.');
            pending.set(url.href,{path});
        }
        return url.href;
    }
    function rewrite(text,base){
        let records;try{[records]=parse(text,base);}catch{fail('Unable to parse module: {path}',new URL(base).pathname);}
        const edits=[];
        for(const record of records){
            if(record.t===3){edits.push({start:record.s,end:record.e,text:`({url:${JSON.stringify(base)}})`});continue;}
            if(![1,2].includes(record.t)||record.a!==-1)fail('Module import attributes and source/defer phases are not supported.');
            const url=resolve(record.n,base);
            edits.push({start:record.t===1?record.s-1:record.s,end:record.t===1?record.e+1:record.e,text:JSON.stringify(url)});
        }
        for(const edit of edits.sort((a,b)=>b.start-a.start))text=text.slice(0,edit.start)+edit.text+text.slice(edit.end);
        return text;
    }
    const entries=scripts.map((script,index)=>{
        let text=script.text;
        if(script.src!==undefined){
            const src=/^(?:[\w+.-]+:|\/|\.\.?\/)/u.test(script.src)?script.src:'./'+script.src;
            if(!script.src)fail('Invalid module path: {path}','');
            text=`import ${JSON.stringify(resolve(src,ORIGIN+entry))};`;
        }else text=rewrite(text,ORIGIN+entry);
        return encode(text)+'#entry-'+index;
    });
    // Map iteration includes dependencies appended while rewriting, without recursive expansion.
    for(const [url,{path}]of pending){
        let text;try{text=new TextDecoder('utf-8',{fatal:true}).decode(files[path].bytes);}catch{fail('Module is not valid UTF-8: {path}',path);}
        imports[url]=encode(rewrite(text,url))+'#'+encodeURIComponent(url);
    }
    return {imports,scripts:entries};
}

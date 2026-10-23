import {test} from 'node:test';
import assert from 'node:assert/strict';
import {moduleGraph} from '../preview-modules.js';
const file=text=>({type:'text/javascript',bytes:new TextEncoder().encode(text)});
const source=url=>Buffer.from(url.split(',')[1].split('#')[0],'base64').toString();
test('module graph preserves cycles and rewrites only lexer-identified imports',async()=>{
    const graph=await moduleGraph({'/a.js':file(`import {b} from './b.js'; export const a=1; const text="import './missing.js'";`),'/b.js':file(`export {a} from './a.js'; export const b=2;`)},[{src:'./a.js'}],'/index.html');
    assert.equal(Object.keys(graph.imports).length,2);
    assert.match(source(graph.imports['https://workspace.invalid/a.js']),/import '\.\/missing.js'/u);
    assert.match(source(graph.imports['https://workspace.invalid/b.js']),/https:\/\/workspace.invalid\/a.js/u);
    assert.match(source(graph.scripts[0]),/import "https:\/\/workspace.invalid\/a.js"/u);
});
test('literal dynamic imports, escaped specifiers and import.meta are resolved locally',async()=>{
    const graph=await moduleGraph({'/sub/a.js':file('export const x=1;')},[{text:`await import('./sub/\\u0061.js'); fetch(new URL('./data.json',import.meta.url));`}],'/index.html');
    const text=source(graph.scripts[0]);assert.match(text,/import\("https:\/\/workspace.invalid\/sub\/a.js"\)/u);assert.match(text,/url:"https:\/\/workspace.invalid\/index.html"/u);
});
test('module graph rejects unsupported dependencies with specific diagnostics',async()=>{
    for(const [text,pattern]of [[`import 'react'`,/npm/u],[`import 'https://cdn.invalid/a.js'`,/CDN/u],[`import('./'+name)`,/dynamic/u],[`import './missing.js'`,/missing.js/u],[`import x from './a.js' with {type:'json'}`,/attributes/u]])
        await assert.rejects(moduleGraph({'/a.js':file('export default 1')},[{text}],'/index.html'),pattern);
});
test('distinct module URLs retain identity even when their source bytes match',async()=>{
    const graph=await moduleGraph({'/a.js':file(`import './same.js?v=1'; import './same.js?v=2';`),'/same.js':file(`globalThis.count=(globalThis.count??0)+1;`)},[{src:'a.js'}],'/index.html');
    assert.notEqual(graph.imports['https://workspace.invalid/same.js?v=1'],graph.imports['https://workspace.invalid/same.js?v=2']);
});
test('module graph bounds URL proliferation and rejects non-JS and invalid UTF-8',async()=>{
    await assert.rejects(moduleGraph({'/a.js':file(Array.from({length:1000},(_,i)=>`import './a.js?v=${i}';`).join(''))},[{src:'a.js'}],'/index.html'),/1000 modules/u);
    await assert.rejects(moduleGraph({'/data.json':{...file('{}'),type:'application/json'}},[{text:`import './data.json'`}],'/index.html'),/Only JavaScript/u);
    await assert.rejects(moduleGraph({'/a.js':{type:'text/javascript',bytes:new Uint8Array([255])}},[{src:'a.js'}],'/index.html'),/UTF-8/u);
});

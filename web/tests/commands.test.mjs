import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {commandCandidates} from '../commands.js';
const catalog=JSON.parse(await readFile(new URL('../commands.json',import.meta.url)));
test('slash candidates match only leading commands and expose real per-platform commands',()=>{
    assert.equal(commandCandidates(catalog,'explain /mode').length,0);assert.equal(commandCandidates(catalog,'/com').length,0);assert.equal(commandCandidates(catalog,'/').some(x=>x.value==='/compact'),false);
    assert.deepEqual(commandCandidates(catalog,'/mo').map(x=>x.value),['/mode']);assert.deepEqual(commandCandidates(catalog,'/mode a').map(x=>x.value),['/mode ask','/mode auto']);assert.equal(commandCandidates(catalog,'/mode auto').length,0);
});
test('dynamic candidates use real session ids and workspace paths, not unrelated data',()=>{
    const result=commandCandidates(catalog,'/resume ',{sessions:[{id:'abc',name:'A name'}]});assert.equal(result[0].value,'/resume abc');
    assert.equal(commandCandidates(catalog,'/download as',{files:[{path:'assets/app.js'}]})[0].value,'/download assets/app.js');assert.equal(commandCandidates(catalog,'/files a',{files:[{path:'a'}]}).length,0);
});
test('reflection command is explicit with only supported role scopes',()=>{
    assert.deepEqual(commandCandidates(catalog,'/ref').map(item=>item.value),['/reflect']);
    assert.deepEqual(commandCandidates(catalog,'/reflect ').map(item=>item.value),['/reflect all','/reflect user','/reflect assistant']);
});

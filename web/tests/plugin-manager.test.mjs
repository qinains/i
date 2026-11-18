import {test} from 'node:test';
import assert from 'node:assert/strict';
import {PluginManager,publicToolName,grantFits} from '../plugins/manager.js';
test('plugin namespace and permission subset cannot enlarge a grant',async()=>{
    const name=await publicToolName('example.word-count','count_words');assert.match(name,/^p_count_words_[a-f0-9]{16}$/u);
    assert.notEqual(name,await publicToolName('other','count_words'));
    assert.equal(grantFits({workspace:'read',storage:false},{workspace:'read-write',storage:true}),true);
    assert.equal(grantFits({workspace:'read-write',storage:false},{workspace:'read',storage:false}),false);
    assert.equal(grantFits({workspace:'none',storage:true},{workspace:'none',storage:false}),false);
    assert.equal(grantFits({workspace:'none',storage:false,network:true},{workspace:'none',storage:false}),false);
});
test('snapshot omits unavailable plugins and logs only the storage stage and error name',async t=>{
    const warning=t.mock.method(console,'warn',()=>{});
    Object.defineProperties(navigator,{
        locks:{configurable:true,value:{request:async(_name,_options,fn)=>fn({})}},
        storage:{configurable:true,value:{getDirectory:async()=>{throw new DOMException('private package path','NotFoundError');}}},
    });
    t.after(()=>{delete navigator.locks;delete navigator.storage;});
    const active={id:'example.count',version:'1.0.0',digest:'a'.repeat(64)};
    const store={loadRegistry:async()=>({revision:7,active:[active],packages:[active],grants:[{workspaceId:'default',pluginId:active.id,version:active.version,digest:active.digest}]})};
    const manager=new PluginManager({store,inspectPackage:()=>assert.fail('missing package cannot be inspected')});
    assert.deepEqual(await manager.withSnapshot('default',snapshot=>snapshot),{workspaceId:'default',revision:7,tools:[],unavailable:[active.id]});
    assert.deepEqual(warning.mock.calls.map(call=>call.arguments),[['[plugin-snapshot] package-path: NotFoundError']]);
});

import {test} from 'node:test';
import assert from 'node:assert/strict';
import {assessPlugin} from '../plugins/policy.js';
import {PluginManager,digest,grantFits} from '../plugins/manager.js';

const moduleDigest='a'.repeat(64),packageBytes=new Uint8Array([1,2,3]);
function verified(permissions={workspace:'none',storage:false}){return {manifest:{format_version:1,id:'example.policy',name:'Policy fixture',version:'1.0.0',runtime:'harness-wasm-v1',entry:'plugin.wasm',sha256:moduleDigest,permissions,tools:[{name:'test',description:'Test tool',parameters:{type:'object'}}]},digest:moduleDigest,license:'MIT',wasm:new Uint8Array()};}
const forbidden={apiCredentials:false,connectionSettings:false,browserStorage:false,network:false,shell:false,imports:false};
function memoryHost(t){
    const previous=Object.getOwnPropertyDescriptor(globalThis,'navigator');let writes=0;
    const directory={getDirectoryHandle:async()=>directory,getFileHandle:async()=>({getFile:async()=>({size:packageBytes.length,arrayBuffer:async()=>packageBytes.slice().buffer}),createWritable:async()=>({write:async()=>{writes++;},close:async()=>{},abort:async()=>{}})})};
    Object.defineProperty(globalThis,'navigator',{configurable:true,value:{locks:{request:async(_name,_options,fn)=>fn({})},storage:{getDirectory:async()=>directory}}});
    t.after(()=>{if(previous)Object.defineProperty(globalThis,'navigator',previous);else delete globalThis.navigator;});
    return ()=>writes;
}
async function stored(t,inspect){
    const writes=memoryHost(t),v=verified(),record={id:v.manifest.id,version:v.manifest.version,digest:v.digest,packageDigest:await digest(packageBytes),manifest:v.manifest,securityChecks:{...forbidden},status:'installed'};
    const state={revision:0,packages:[record],active:[{id:record.id,version:record.version,digest:record.digest}],grants:[{workspaceId:'default',pluginId:record.id,version:record.version,digest:record.digest,permissions:v.manifest.permissions}]};
    const commits=[],store={loadRegistry:async()=>state,commitRegistry:async change=>{commits.push(change);}};
    return {manager:new PluginManager({store,inspectPackage:inspect}),commits,writes,record};
}
test('host capability report distinguishes scoped plugin storage from browser storage',()=>{
    for(const workspace of ['none','read','read-write'])for(const storage of [false,true]){
        const report=assessPlugin(verified({workspace,storage}));
        assert.deepEqual(report,{workspace,pluginStorage:storage,...forbidden});assert.ok(Object.isFrozen(report));
        assert.throws(()=>{report.apiCredentials=true;},TypeError);
    }
});
test('host policy rejects undeclared sensitive capabilities and self-authored reports',()=>{
    for(const key of ['api_key','credentials','apiCredentials','connectionSettings','network','browserStorage','shell','imports']){
        const v=verified();v.manifest.permissions[key]=false;assert.throws(()=>assessPlugin(v));
    }
    for(const key of ['securityChecks','report','api_key','credentials']){const v=verified();v.manifest[key]={safe:true};assert.throws(()=>assessPlugin(v));}
    for(const key of ['securityChecks','report'])assert.throws(()=>assessPlugin({...verified(),[key]:{safe:true}}));
    for(const permissions of [{workspace:'admin',storage:false},{workspace:'none',storage:1},{workspace:'none'},Object.assign(Object.create({workspace:'none'}),{storage:false})])assert.throws(()=>assessPlugin(verified(permissions)));
});
test('host policy requires known format/runtime/entry and verified module digest',()=>{
    for(const [key,value]of [['format_version',2],['runtime','javascript'],['entry','main.js'],['sha256','b'.repeat(64)],['sha256','A'.repeat(64)]]){const v=verified();v.manifest[key]=value;assert.throws(()=>assessPlugin(v));}
    for(const value of [undefined,'fake','A'.repeat(64),'b'.repeat(64)])assert.throws(()=>assessPlugin({...verified(),digest:value}));
});
test('manager preview attaches host report only after successful trusted inspection',async()=>{
    const manager=new PluginManager({store:{},inspectPackage:async()=>verified({workspace:'read',storage:true})});
    const preview=await manager.preview(packageBytes);assert.deepEqual(preview.securityChecks,{workspace:'read',pluginStorage:true,...forbidden});assert.ok(Object.isFrozen(preview.securityChecks));
    const invalid=verified();invalid.manifest.permissions.credentials=true;
    await assert.rejects(new PluginManager({store:{},inspectPackage:async()=>invalid}).preview(packageBytes));
    await assert.rejects(new PluginManager({store:{},inspectPackage:async()=>{throw new Error('invalid WASM');}}).preview(packageBytes),/invalid WASM/);
});
test('production inspector result may carry trusted worker-generated descriptors',async()=>{
    const result=verified({workspace:'read',storage:true});
    result.descriptors=[{pluginId:result.manifest.id,version:result.manifest.version,digest:result.digest,localName:'test',publicName:'p_test_fixture',description:'Test tool',parameters:{type:'object'},permissions:result.manifest.permissions}];
    assert.deepEqual(Object.keys(result).sort(),['manifest','digest','license','wasm','descriptors'].sort());
    const manager=new PluginManager({store:{},inspectPackage:async()=>result});
    assert.equal((await manager.preview(packageBytes)).securityChecks.workspace,'read');
    const untrusted=verified();untrusted.manifest.descriptors=result.descriptors;assert.throws(()=>assessPlugin(untrusted));
});
test('install reinspects policy rather than trusting modified preview report',async t=>{
    const writes=memoryHost(t);let calls=0,commits=0;
    const manager=new PluginManager({store:{loadRegistry:async()=>({revision:0,packages:[],active:[],grants:[]}),commitRegistry:async()=>{commits++;}},inspectPackage:async()=>{const v=verified();if(++calls>1)v.manifest.permissions.network=true;return v;}});
    const preview=await manager.preview(packageBytes);preview.securityChecks={network:false};
    await assert.rejects(manager.install(preview));assert.equal(calls,2);assert.equal(writes(),0);assert.equal(commits,0);
});
test('successful installation does not persist the advisory report as an authority',async t=>{
    const writes=memoryHost(t),commits=[];let inspections=0;
    const manager=new PluginManager({store:{loadRegistry:async()=>({revision:0,packages:[],active:[],grants:[]}),commitRegistry:async change=>{commits.push(change);}},inspectPackage:async()=>{inspections++;return verified();}});
    const result=await manager.install(await manager.preview(packageBytes));
    assert.equal(inspections,3);assert.equal(writes(),1);assert.equal(result.securityChecks,undefined);assert.equal(commits[0].putPackages[0].securityChecks,undefined);
});
test('enable rejects old installed packages whose new inspection violates host policy',async t=>{
    const invalid=verified();invalid.manifest.permissions.api_key=true;
    const {manager,commits,writes}=await stored(t,async()=>invalid);
    await assert.rejects(manager.enable('default','example.policy',{workspace:'none',storage:false}));assert.equal(commits.length,0);assert.equal(writes(),0);
    const snapshot=await manager.withSnapshot('default',value=>value);assert.deepEqual(snapshot.tools,[]);assert.deepEqual(snapshot.unavailable,['example.policy']);
    assert.equal((await manager.list())[0].status,'unavailable');
});
test('list recomputes reports and enable preserves declared grant limits',async t=>{
    const {manager,commits}=await stored(t,async()=>verified());
    const [item]=await manager.list();assert.deepEqual(item.securityChecks,{workspace:'none',pluginStorage:false,...forbidden});
    await assert.rejects(manager.enable('default','example.policy',{workspace:'read',storage:false}));assert.equal(commits.length,0);
    await manager.enable('default','example.policy',{workspace:'none',storage:false});assert.equal(commits.length,1);
    assert.equal(grantFits({workspace:'read',storage:true},{workspace:'read',storage:false}),false);
    assert.equal(grantFits({workspace:'none',storage:false,api_key:false},{workspace:'none',storage:false}),false);
    assert.equal(grantFits(Object.assign(Object.create({workspace:'read'}),{storage:false,extra:false}),{workspace:'read',storage:false}),false);
});

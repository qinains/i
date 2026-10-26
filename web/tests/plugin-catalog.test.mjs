import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {CATALOG_URL,validateCatalog,fetchCatalog,previewCatalogPlugin} from '../plugins/catalog.js';

const bytes=new TextEncoder().encode('verified package fixture');
const sha256=createHash('sha256').update(bytes).digest('hex');
function entry(){return {id:'example.words',name:'Words',version:'1.2.3',description:'Count words',repository:'https://github.com/qinains/i-plugins',source_revision:'a'.repeat(40),license:'MIT',package:{url:`https://qinains.github.io/i-plugins/packages/${sha256}.hplugin.zip`,sha256,size:bytes.length},permissions:{workspace:'read',storage:false}};}
function catalog(plugin=entry()){return {schema_version:1,plugins:[plugin]};}
function response(body=JSON.stringify(catalog()),type='application/json',headers={}){return new Response(body,{headers:{'content-type':type,...headers}});}
function manager(overrides={}){return {preview:async value=>{assert.deepEqual(value,bytes);const e=entry();return {manifest:{id:e.id,name:e.name,version:e.version,permissions:e.permissions}};},install(){assert.fail('must not install');},enable(){assert.fail('must not enable');},...overrides};}

test('valid catalog is sanitized, deeply immutable and detached',()=>{
    const input=catalog();input.plugins[0].ignored='untrusted';const safe=validateCatalog(input);
    assert.equal(safe.plugins[0].ignored,undefined);assert.ok(Object.isFrozen(safe.plugins[0].package));assert.ok(Object.isFrozen(safe.plugins));
    input.plugins[0].name='Changed';assert.equal(safe.plugins[0].name,'Words');
    assert.equal(validateCatalog(catalog({...entry(),id:'constructor'})).plugins[0].id,'constructor');
});
test('reject invalid schema, duplicates, identities, metadata and permissions',()=>{
    for(const value of [null,{}, {...catalog(),schema_version:2},{schema_version:1,plugins:Array(501).fill(entry())},{schema_version:1,plugins:[entry(),entry()]}])assert.throws(()=>validateCatalog(value));
    for(const patch of [{id:'builtin.foo'},{id:'Bad'},{version:'01.2.3'},{version:'1.2.3-beta'},{name:''},{description:'x'.repeat(4097)},{repository:'https://github.com/a/b?token=x'},{source_revision:'A'.repeat(40)},{permissions:{workspace:'none',storage:false,network:true}}])assert.throws(()=>validateCatalog(catalog({...entry(),...patch})));
    const polluted=JSON.parse(JSON.stringify(catalog()).replace('"schema_version":1','"__proto__":{},"schema_version":1'));assert.throws(()=>validateCatalog(polluted));
    assert.throws(()=>validateCatalog(Object.assign(Object.create({schema_version:1}),{plugins:[entry()]})));
    assert.throws(()=>validateCatalog({schema_version:1,plugins:Array.from({length:130},(_,i)=>({...entry(),id:`example.${i}`,description:'x'.repeat(4096)}))}));
});
test('package URL is exact trusted digest-addressed mirror only',()=>{
    const e=entry();for(const url of [e.package.url.replace('qinains.github.io','evil.test'),e.package.url.replace('https://','http://'),e.package.url.replace('https://','https://user:pass@'),e.package.url+'?x=1',e.package.url+'#x',e.package.url.replace(sha256,'b'.repeat(64)),e.package.url.replace('/packages/','/packages/../packages/'),e.package.url.replace('packages','%70ackages')])assert.throws(()=>validateCatalog(catalog({...e,package:{...e.package,url}})));
    for(const size of [0,-1,10*1024*1024+1,1.5])assert.throws(()=>validateCatalog(catalog({...e,package:{...e.package,size}})));
});
test('metadata uses UTF-8 byte limits and version components fit u32',()=>{
    for(const [field,limit]of [['name',256],['description',4096],['license',128]]){
        const exact='é'.repeat(limit/2);assert.equal(validateCatalog(catalog({...entry(),[field]:exact})).plugins[0][field],exact);
        assert.throws(()=>validateCatalog(catalog({...entry(),[field]:exact+'a'})));
        assert.throws(()=>validateCatalog(catalog({...entry(),[field]:''})));
        assert.throws(()=>validateCatalog(catalog({...entry(),[field]:'   '})));
    }
    assert.equal(validateCatalog(catalog({...entry(),version:'4294967295.4294967295.4294967295'})).plugins[0].version,'4294967295.4294967295.4294967295');
    for(const version of ['4294967296.0.0','0.4294967296.0','0.0.4294967296'])assert.throws(()=>validateCatalog(catalog({...entry(),version})));
});
test('catalog and package requests omit ambient credentials and never install',async()=>{
    const calls=[];const fetchImpl=async(url,options)=>{calls.push([url,options]);return url===CATALOG_URL?response():response(bytes,'application/zip');};
    const result=await fetchCatalog({fetchImpl});const preview=await previewCatalogPlugin(manager(),result.plugins[0],{fetchImpl});assert.equal(preview.manifest.id,entry().id);
    assert.equal(calls.length,2);assert.equal(calls[0][0],'https://qinains.github.io/i-plugins/index.json');
    for(const [,options]of calls){assert.equal(options.credentials,'omit');assert.equal(options.cache,'no-store');assert.equal(options.referrerPolicy,'no-referrer');assert.equal(options.redirect,'error');assert.equal(options.mode,'cors');assert.ok(options.signal instanceof AbortSignal);assert.equal(options.headers,undefined);}
});
test('catalog rejects HTTP, MIME, malformed JSON, redirect and fetch failures',async()=>{
    for(const fetchImpl of [async()=>new Response('',{status:403}),async()=>response('{}','text/html'),async()=>response('{}','application/jsonx'),async()=>response('broken'),async()=>({ok:true,redirected:true,headers:new Headers(),body:null}),async()=>{throw new Error('offline');}])await assert.rejects(fetchCatalog({fetchImpl}));
});
test('catalog rejects oversized declared and streamed bodies',async()=>{
    await assert.rejects(fetchCatalog({fetchImpl:async()=>response('{}','application/json',{'content-length':String(512*1024+1)})}));
    let cancelled=false;const body=new ReadableStream({start(controller){controller.enqueue(new Uint8Array(512*1024+1));},cancel(){cancelled=true;}});
    await assert.rejects(fetchCatalog({fetchImpl:async()=>response(body)}));assert.equal(cancelled,true);
});
test('compressed responses bound decoded bytes rather than comparing their wire length',async()=>{
    const headers={'content-encoding':'gzip','content-length':'12'};
    assert.equal((await fetchCatalog({fetchImpl:async()=>response(JSON.stringify(catalog()),'application/json',headers)})).plugins[0].id,entry().id);
    await previewCatalogPlugin(manager(),entry(),{fetchImpl:async()=>response(bytes,'application/zip',headers)});
    // Cross-origin Fetch may hide Content-Encoding even for a decoded gzip body.
    assert.equal((await fetchCatalog({fetchImpl:async()=>response(JSON.stringify(catalog()),'application/json',{'content-length':'12'})})).plugins[0].id,entry().id);
    await previewCatalogPlugin(manager(),entry(),{fetchImpl:async()=>response(bytes,'application/zip',{'content-length':'12'})});
    await previewCatalogPlugin(manager(),entry(),{fetchImpl:async()=>response(bytes,'application/zip',{'content-length':String(bytes.length+1)})});
    await assert.rejects(fetchCatalog({fetchImpl:async()=>response('x'.repeat(512*1024+1),'application/json',headers)}));
    await assert.rejects(fetchCatalog({fetchImpl:async()=>response('{}','application/json',{...headers,'content-length':String(512*1024+1)})}));
    for(const payload of [bytes.slice(1),new Uint8Array(bytes.length),new Uint8Array(bytes.length+1)])await assert.rejects(previewCatalogPlugin(manager(),entry(),{fetchImpl:async()=>response(payload,'application/zip',headers)}));
});
test('package enforces length, digest, direct entry validation and manifest identity',async()=>{
    let inspected=false;const m=manager({preview:async()=>{inspected=true;return {};}});
    for(const payload of [bytes.slice(1),new Uint8Array(bytes.length),new Uint8Array(bytes.length+1)]){await assert.rejects(previewCatalogPlugin(m,entry(),{fetchImpl:async()=>response(payload)}));assert.equal(inspected,false);}
    await assert.rejects(previewCatalogPlugin(m,entry(),{fetchImpl:async()=>response(bytes,'application/zip',{'content-length':String(10*1024*1024+1)})}));
    await assert.rejects(previewCatalogPlugin(m,{...entry(),id:'builtin.bad'},{fetchImpl:async()=>assert.fail('invalid entry must not fetch')}));
    for(const patch of [{id:'other'},{name:'Other'},{version:'9.0.0'},{permissions:{workspace:'read-write',storage:false}},{permissions:{workspace:'read',storage:false,network:true}}])await assert.rejects(previewCatalogPlugin(manager({preview:async()=>({manifest:{...entry(),...patch}})}),entry(),{fetchImpl:async()=>response(bytes)}));
});
test('entry snapshot cannot be changed during download',async()=>{
    const e=entry();await previewCatalogPlugin(manager(),e,{fetchImpl:async()=>{e.name='evil';e.package.sha256='b'.repeat(64);e.permissions.workspace='read-write';return response(bytes);}});
});
test('caller cancellation covers hanging fetch and streaming reads',async()=>{
    for(const fetchImpl of [async()=>new Promise(()=>{}),async()=>response(new ReadableStream())]){const controller=new AbortController();const pending=fetchCatalog({fetchImpl,signal:controller.signal});queueMicrotask(()=>controller.abort());await assert.rejects(pending,{name:'AbortError'});}
    let cancel=false;const controller=new AbortController();let reading;
    const started=new Promise(resolve=>{reading=resolve;});
    const stream=new ReadableStream({pull(){reading();},cancel(){cancel=true;}});
    const pending=fetchCatalog({fetchImpl:async()=>response(stream),signal:controller.signal});await started;await new Promise(resolve=>setImmediate(resolve));controller.abort();await assert.rejects(pending,{name:'AbortError'});assert.equal(cancel,true);
    const aborted=new AbortController();aborted.abort();await assert.rejects(fetchCatalog({signal:aborted.signal,fetchImpl:async()=>assert.fail('pre-aborted fetch')}),{name:'AbortError'});
    const duringFetch=new AbortController();await assert.rejects(fetchCatalog({signal:duringFetch.signal,fetchImpl:()=>{duringFetch.abort();return Promise.reject(new Error('fetch aborted'));}}),{name:'AbortError'});
});
test('timeout is bounded at 30 seconds and aborts hanging fetch',async()=>{
    const original=globalThis.setTimeout;let delay;globalThis.setTimeout=(fn,ms)=>{delay=ms;return original(fn,1);};
    try{await assert.rejects(fetchCatalog({fetchImpl:async()=>new Promise(()=>{})}),{name:'TimeoutError'});assert.equal(delay,30000);}finally{globalThis.setTimeout=original;}
});

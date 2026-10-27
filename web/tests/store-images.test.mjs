import test from 'node:test';
import assert from 'node:assert/strict';
import {openStateStore} from '../state/store.js';
import {SessionController} from '../sessions.js';
const MiB=1024*1024;
const record=extra=>({id:'image',workspaceId:'default',name:'Saved',titleSource:'manual',connectionId:'',connectionIdentity:'',draft:'',revision:1,history:[],transcript:[],status:'idle',...extra});
const image=blob=>({kind:'image',token:'[Image #1]',blob});
function storage(t,sessions,fresh=()=>sessions()[0]){
    const previous=globalThis.indexedDB;let writes=0;
    globalThis.indexedDB={open(){
        const request={};queueMicrotask(()=>{
            request.result={close(){},transaction(name){
                const tx={objectStore:()=>({
                    getAll(){const result={};queueMicrotask(()=>{result.result=name==='sessions'?sessions():[];tx.oncomplete();});return result;},
                    get(){const result={};queueMicrotask(()=>{result.result=fresh();tx.oncomplete();});return result;},
                    put(){writes++;},delete(){writes++;},
                })};return tx;
            }};request.onsuccess();
        });return request;
    }};
    t.after(()=>{globalThis.indexedDB=previous;});return ()=>writes;
}
test('metadata load is lazy; opened draft and queued images own bytes independently of IDB',async t=>{
    let readable=true,reads=0;
    class StoredBlob extends Blob{arrayBuffer(){reads++;return readable?super.arrayBuffer():Promise.reject(new DOMException('Backing snapshot replaced','NotFoundError'));}}
    const draft=new StoredBlob(['draft'],{type:'image/png'}),queued=new StoredBlob(['queued'],{type:'image/jpeg'});
    const saved=record({composerBlocks:[image(draft)],pendingMessages:[{id:'queued',visible:'[Image #1]',connectionId:'',connectionIdentity:'',blocks:[image(queued)]}]});
    const writes=storage(t,()=>[saved]),store=await openStateStore();try{
        const {sessions:[loaded]}=await store.load();assert.equal(reads,0);
        const {record:opened,bytes}=await store.prepareSession(loaded,128*MiB);assert.equal(bytes,11);readable=false;
        assert.notEqual(opened.composerBlocks[0].blob,draft);assert.notEqual(opened.pendingMessages[0].blocks[0].blob,queued);
        assert.equal(await opened.composerBlocks[0].blob.text(),'draft');assert.equal(opened.composerBlocks[0].blob.type,'image/png');
        assert.equal(await opened.pendingMessages[0].blocks[0].blob.text(),'queued');assert.equal(opened.pendingMessages[0].blocks[0].blob.type,'image/jpeg');assert.equal(writes(),0);
    }finally{store.close();}
});
test('legacy records open without attachments and invalid shape is rejected before reading bytes',async t=>{
    let reads=0;class StoredBlob extends Blob{arrayBuffer(){reads++;return super.arrayBuffer();}}
    const legacy=record({id:'legacy'}),invalid=record({id:'invalid',composerBlocks:{},pendingMessages:[]});
    storage(t,()=>[legacy,invalid]);const store=await openStateStore();try{
        assert.deepEqual((await store.load()).sessions,[legacy,invalid]);assert.equal((await store.prepareSession(legacy,128*MiB)).bytes,0);
        await assert.rejects(store.prepareSession(invalid,128*MiB),/attachments/);
        await assert.rejects(store.prepareSession(record({composerBlocks:[image(new StoredBlob(['x']))],history:null}),128*MiB),/Invalid saved session/);assert.equal(reads,0);
    }finally{store.close();}
});
test('one unreadable image isolates its session and never writes or deletes stored records',async t=>{
    class UnreadableBlob extends Blob{arrayBuffer(){return Promise.reject(new DOMException('Unreadable fixture','NotFoundError'));}}
    const original=new UnreadableBlob(['retain'],{type:'image/png'}),readable=new Blob(['keep'],{type:'image/png'});
    const broken=record({composerBlocks:[image(readable),{...image(original),token:'[Image #2]'}]}),healthy=record({id:'healthy'});
    const writes=storage(t,()=>[broken,healthy],()=>broken),store=await openStateStore(),errors=[];
    try{
        const controller=new SessionController({},store,{error:error=>errors.push(error.message)});await controller.restore((await store.load()).sessions);
        assert.equal(controller.records.has('image'),false);assert.equal(controller.records.has('healthy'),true);
        assert.match(errors[0],/retained in storage.*stage: attachments/);assert.equal(broken.composerBlocks[0].blob,readable);assert.equal(broken.composerBlocks[1].blob,original);assert.equal(writes(),0);
    }finally{store.close();}
});
test('image sizes and remaining restore budget are checked before any byte reads',async t=>{
    let reads=0;class SizedBlob extends Blob{get size(){return 8*MiB;}arrayBuffer(){reads++;return super.arrayBuffer();}}
    class OversizedBlob extends SizedBlob{get size(){return 8*MiB+1;}}
    storage(t,()=>[]);const store=await openStateStore();try{
        await assert.rejects(store.prepareSession(record({composerBlocks:[image(new OversizedBlob(['x']))]}),128*MiB),/oversized/);
        await assert.rejects(store.prepareSession(record({composerBlocks:[image(new SizedBlob(['x']))]}),MiB),/memory budget/);
        await assert.rejects(store.prepareSession(record({composerBlocks:Array.from({length:3},()=>image(new SizedBlob(['x'])))}),128*MiB),/draft images/);
        assert.equal(reads,0);
    }finally{store.close();}
});
test('a retired IDB image handle gets only one fresh read at the same revision',async t=>{
    class RetiredBlob extends Blob{arrayBuffer(){return Promise.reject(new DOMException('Retired fixture','NotFoundError'));}}
    const old=record({composerBlocks:[image(new RetiredBlob(['old']))]}),fresh=record({composerBlocks:[image(new Blob(['fresh'],{type:'image/png'}))]});let retries=0;
    storage(t,()=>[old],()=>{retries++;return fresh;});const store=await openStateStore();try{
        const prepared=await store.prepareSession(old,128*MiB);assert.equal(retries,1);assert.equal(await prepared.record.composerBlocks[0].blob.text(),'fresh');
        fresh.revision++;
        await assert.rejects(store.prepareSession(old,128*MiB),{name:'NotFoundError'});assert.equal(retries,2);
    }finally{store.close();}
});
test('restore bounds total retained image bytes and leaves over-budget sessions unopened',async()=>{
    const errors=[],budgets=[];
    const store={prepareSession:async(record,budget)=>{budgets.push(budget);if(budget<80*MiB)throw new Error('budget');return {record,bytes:80*MiB};}};
    const controller=new SessionController({},store,{error:error=>errors.push(error.message)});
    await controller.restore([record({id:'first'}),record({id:'second'})]);
    assert.deepEqual(budgets,[128*MiB,48*MiB]);assert.deepEqual([...controller.records.keys()],['first']);assert.match(errors[0],/stage: attachments/);
});
test('prepared bytes count toward the restore budget even if later history recovery fails',async()=>{
    const budgets=[];
    const store={prepareSession:async(record,budget)=>{budgets.push(budget);return {record,bytes:80*MiB};}};
    const controller=new SessionController({validate_history(){throw new Error('Invalid history');}},store,{error(){}});
    await controller.restore([record({id:'invalid-history',history:[{role:'system',content:'bad'}]}),record({id:'healthy'})]);
    assert.deepEqual(budgets,[128*MiB,48*MiB]);assert.deepEqual([...controller.records.keys()],['healthy']);
});

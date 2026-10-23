import test from 'node:test';
import assert from 'node:assert/strict';
import {FileRefresh} from '../file-refresh.js';
test('a late older directory listing cannot overwrite the latest result',async()=>{
    const refresh=new FileRefresh(),seen=[];let release;
    const old=refresh.run(()=>new Promise(resolve=>release=resolve),value=>seen.push(value));
    assert.equal(await refresh.run(async()=>['new'],value=>seen.push(value)),true);
    release(['old']);assert.equal(await old,false);assert.deepEqual(seen,[['new']]);
});
test('cancellation aborts a queued read without publishing an error or stale result',async()=>{
    const refresh=new FileRefresh();let applied=false;
    const pending=refresh.run(signal=>new Promise((resolve,reject)=>signal.addEventListener('abort',()=>reject(signal.reason),{once:true})),()=>applied=true);
    refresh.cancel();assert.equal(await pending,false);assert.equal(applied,false);
});
test('an already cancelled refresh never starts its loader',async()=>{
    const refresh=new FileRefresh(),abort=new AbortController();abort.abort();let loaded=false;
    assert.equal(await refresh.run(async()=>loaded=true,()=>{},abort.signal),false);assert.equal(loaded,false);
});
test('a failed latest refresh preserves previous displayed data and rejects its real error',async()=>{
    const refresh=new FileRefresh();let shown=['saved'];
    await assert.rejects(refresh.run(async()=>{throw new Error('denied');},value=>shown=value),/denied/);
    assert.deepEqual(shown,['saved']);
});

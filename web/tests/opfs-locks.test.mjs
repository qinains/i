import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from '../platform.js';
const id='temp-12345678-1234-4123-8123-123456789af0';
test('conditional writes reject changed contents and unexpected file creation',async()=>{
    assert.equal(typeof fs.opfsFileVersion,'function');assert.equal(typeof fs.opfsWriteIfVersion,'function');
    const root=await fs.opfsOpenWorkspace(id);await fs.opfsWrite(root,'versioned',new Uint8Array([1]));
    const version=await fs.opfsFileVersion(root,'versioned');assert.match(version,/^[a-f0-9]{64}$/u);
    await fs.opfsWrite(root,'versioned',new Uint8Array([2]));
    await assert.rejects(fs.opfsWriteIfVersion(root,'versioned',new Uint8Array([3]),version),/changed/i);
    assert.deepEqual([...await fs.opfsRead(root,'versioned',10)],[2]);
    const current=await fs.opfsFileVersion(root,'versioned');await fs.opfsWriteIfVersion(root,'versioned',new Uint8Array([4]),current);
    assert.deepEqual([...await fs.opfsRead(root,'versioned',10)],[4]);
    assert.equal(await fs.opfsFileVersion(root,'new-versioned'),null);await fs.opfsWrite(root,'new-versioned',new Uint8Array([5]));
    await assert.rejects(fs.opfsWriteIfVersion(root,'new-versioned',new Uint8Array([6]),null),/changed/i);
    assert.deepEqual([...await fs.opfsRead(root,'new-versioned',10)],[5]);
});
test('nested scopes respect an explicitly aborted signal without releasing their parent',async()=>{
    const root=await fs.opfsOpenWorkspace(id),abort=new AbortController();abort.abort();let ran=false;
    await fs.opfsWithTransaction(root,'exclusive',async scoped=>{
        await assert.rejects(fs.opfsWithTransaction(scoped,'exclusive',async()=>ran=true,abort.signal),/abort/i);
        await fs.opfsWrite(scoped,'parent-still-active',new Uint8Array([9]));
    });
    assert.equal(ran,false);
});
test('overlapping low-level writes never open concurrent writers',async()=>{
    const root=await fs.opfsOpenWorkspace(id),original=root.getFileHandle.bind(root);
    let release,entered;const gate=new Promise(resolve=>release=resolve),started=new Promise(resolve=>entered=resolve);let active=0,maximum=0,count=0;
    root.getFileHandle=async(...args)=>{const file=await original(...args);return {...file,createWritable:async()=>{
        const writer=await file.createWritable(),first=count++===0;maximum=Math.max(maximum,++active);
        return {write:async bytes=>{if(first){entered();await gate;}await writer.write(bytes);},close:async()=>{try{await writer.close();}finally{active--;}},abort:()=>writer.abort()};
    }};};
    try{
        const first=fs.opfsWrite(root,'same',new Uint8Array([1]));await started;
        const second=fs.opfsWrite(root,'same',new Uint8Array([2]));await new Promise(resolve=>setImmediate(resolve));
        const observed=maximum;release();await Promise.all([first,second]);assert.equal(observed,1);
        assert.deepEqual([...await fs.opfsRead(root,'same',10)],[2]);
    }finally{release();root.getFileHandle=original;}
});
test('explicit leases support nested reads, reject writes in read scopes and expire on return',async()=>{
    assert.equal(typeof fs.opfsWithTransaction,'function');const root=await fs.opfsOpenWorkspace(id);let expired;
    await fs.opfsWithTransaction(root,'shared',async lease=>{
        expired=lease;await assert.rejects(fs.opfsWrite(lease,'forbidden',new Uint8Array([1])),/read.only/i);
        await fs.opfsWithTransaction(lease,'shared',async nested=>assert.deepEqual([...await fs.opfsRead(nested,'same',10)],[2]));
    });
    await assert.rejects(fs.opfsRead(expired,'same',10),/closed/i);
});
test('transaction errors release the gate without leaking write permission',async()=>{
    assert.equal(typeof fs.opfsWithTransaction,'function');const root=await fs.opfsOpenWorkspace(id);let expired;
    await assert.rejects(fs.opfsWithTransaction(root,'exclusive',lease=>{expired=lease;throw new Error('injected');}),/injected/);
    await fs.opfsWrite(root,'after-error',new Uint8Array([3]));
    await assert.rejects(fs.opfsWrite(expired,'after-error',new Uint8Array([4])),/closed/i);
    assert.deepEqual([...await fs.opfsRead(root,'after-error',10)],[3]);
});
test('returning early from a scope does not unlock an in-flight file write',async()=>{
    const root=await fs.opfsOpenWorkspace(id),original=root.getFileHandle.bind(root);let release,entered;
    const gate=new Promise(resolve=>release=resolve),started=new Promise(resolve=>entered=resolve);let pending,done=false;
    root.getFileHandle=async(...args)=>{const file=await original(...args);return {...file,createWritable:async()=>{const writer=await file.createWritable();return {...writer,write:async bytes=>{entered();await gate;return writer.write(bytes);}};}};};
    try{
        const task=fs.opfsWithTransaction(root,'exclusive',async lease=>{pending=fs.opfsWrite(lease,'inflight',new Uint8Array([8]));await started;}).then(()=>done=true);
        await started;await new Promise(resolve=>setImmediate(resolve));const early=done;release();await task;await pending;assert.equal(early,false);
    }finally{release();root.getFileHandle=original;}
});
test('cancelling a granted lease still drains active I/O before admitting the next writer',async()=>{
    const root=await fs.opfsOpenWorkspace(id),original=root.getFileHandle.bind(root);let release,entered,count=0,nextDone=false;
    const gate=new Promise(resolve=>release=resolve),started=new Promise(resolve=>entered=resolve),abort=new AbortController();
    root.getFileHandle=async(...args)=>{const file=await original(...args);return {...file,createWritable:async()=>{const writer=await file.createWritable(),first=count++===0;return {...writer,write:async bytes=>{if(first){entered();await gate;}return writer.write(bytes);}};}};};
    try{
        const lease=await fs.opfsBegin(root,'exclusive',abort.signal),first=fs.opfsWrite(lease,'cancel-inflight',new Uint8Array([1]));
        await started;abort.abort();const next=fs.opfsWrite(root,'cancel-inflight',new Uint8Array([2])).then(()=>nextDone=true);
        await new Promise(resolve=>setImmediate(resolve));const early=nextDone;
        await assert.rejects(fs.opfsWrite(lease,'forbidden',new Uint8Array([3])),/closed/);
        release();await first;await next;assert.equal(early,false);assert.deepEqual([...await fs.opfsRead(root,'cancel-inflight',10)],[2]);
    }finally{release();root.getFileHandle=original;}
});

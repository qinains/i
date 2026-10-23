import test from 'node:test';
import assert from 'node:assert/strict';
import * as platform from '../platform.js';
import * as paths from '../workspaces.js';
import {previewSnapshot} from '../preview.js';
test('browser mount paths preserve relative tmp and reject escaping absolute paths',()=>{
    assert.equal(typeof paths.browserPath,'function');
    assert.deepEqual(paths.browserPath('/tmp/a.txt'),{mount:'tmp',path:'a.txt'});
    assert.deepEqual(paths.browserPath('tmp/a.txt'),{mount:'workspace',path:'tmp/a.txt'});
    assert.deepEqual(paths.browserPath('/workspace/a.txt'),{mount:'workspace',path:'a.txt'});
    for(const path of ['/etc/passwd','/tmp/../x','/tmpx/x','C:/x','a\\b'])assert.throws(()=>paths.browserPath(path));
});
test('temporary root and tmp mount share within workspace, not across workspaces',async()=>{
    const a='temp-12345678-1234-4123-8123-123456789abc',b='temp-12345678-1234-4123-8123-123456789abd';
    const root=await platform.opfsOpenWorkspace(a);await platform.opfsWrite(root,'a.txt',new Uint8Array([1,2]));
    assert.deepEqual([...await platform.opfsRead(await platform.opfsOpenWorkspace(a),'a.txt',10)],[1,2]);
    await platform.opfsWrite(root,'/tmp/b.txt',new Uint8Array([3]));await platform.opfsWrite(root,'tmp/b.txt',new Uint8Array([4]));
    assert.equal(await platform.opfsMetadata(await platform.opfsOpenWorkspace(b),'a.txt'),null);
    await platform.opfsClearTmp(a);assert.equal(await platform.opfsMetadata(root,'/tmp/b.txt'),null);
    assert.deepEqual([...await platform.opfsRead(root,'tmp/b.txt',10)],[4]);
});
test('concurrent temporary opens share one root',async()=>{
    const [a,b]=await Promise.all([1,2].map(()=>platform.opfsOpenWorkspace('temp-12345678-1234-4123-8123-123456789abe')));assert.equal(a,b);
});
test('memory quota failure preserves the previous committed file',async()=>{
    const root=await platform.opfsOpenWorkspace('temp-12345678-1234-4123-8123-123456789abf');
    for(let i=0;i<4;i++)await platform.opfsWrite(root,`block${i}`,new Uint8Array(8*1024*1024));
    await assert.rejects(platform.opfsWrite(root,'/tmp/too-big',new Uint8Array(8*1024*1024+1)),/8 MiB/);
    await platform.opfsWrite(root,'block0',new Uint8Array([7]));await platform.opfsWrite(root,'extra',new Uint8Array(8*1024*1024-1));
    await assert.rejects(platform.opfsWrite(root,'block0',new Uint8Array([8,9])),/32 MiB/);
    assert.deepEqual([...await platform.opfsRead(root,'block0',10)],[7]);
});
test('preview snapshot enforces aggregate bytes and contains only the selected mount',async()=>{
    const id='temp-12345678-1234-4123-8123-123456789ac0',root=await platform.opfsOpenWorkspace(id);
    await platform.opfsWrite(root,'index.html',new TextEncoder().encode('<h1>test</h1>'));await platform.opfsWrite(root,'/tmp/index.html',new Uint8Array([1]));
    assert.deepEqual(Object.keys((await previewSnapshot(id,'index.html')).files),['/index.html']);
    assert.equal((await previewSnapshot(id,'/tmp/index.html')).files['/index.html'].bytes[0],1);
    await assert.rejects(previewSnapshot(id,'missing.html'),/not found/);
    for(let i=0;i<2;i++)await platform.opfsWrite(root,`large${i}`,new Uint8Array(8*1024*1024));
    await assert.rejects(previewSnapshot(id,'index.html'),/16 MiB/);
});
test('temporary workspace and tmp share one quota and clearing tmp does not reset workspace usage',async()=>{
    const id='temp-12345678-1234-4123-8123-123456789ac1',root=await platform.opfsOpenWorkspace(id),block=new Uint8Array(8*1024*1024);
    await platform.opfsWrite(root,'workspace.bin',block);for(let i=0;i<3;i++)await platform.opfsWrite(root,`/tmp/block${i}`,block);
    await assert.rejects(platform.opfsWrite(root,'overflow.bin',new Uint8Array([1])),/32 MiB/);
    await platform.opfsClearTmp(id);for(let i=0;i<3;i++)await platform.opfsWrite(root,`new${i}`,block);
    await assert.rejects(platform.opfsWrite(root,'/tmp/overflow.bin',new Uint8Array([1])),/32 MiB/);
    assert.equal((await platform.opfsMetadata(root,'workspace.bin')).len,block.length);
});

import {test,expect} from './fixtures.mjs';
async function ready(page){await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');}
test('AI commits appear before the next model response and survive repeated reloads',async({page})=>{
    await ready(page);
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();
    let release;const gate=new Promise(resolve=>release=resolve);
    await page.route('**/mock/v1/chat/completions',async route=>{
        const first=route.request().postDataJSON().messages.at(-1).role==='user';
        if(!first)await gate;
        const delta=first?{tool_calls:[{index:0,id:'durable-write',type:'function',function:{name:'write',arguments:JSON.stringify({path:'nested/durable.txt',content:'committed by WASM'})}}]}:{content:'Done'};
        await route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:first?'tool_calls':'stop'}]})}\n\ndata: [DONE]\n\n`});
    });
    try{
        await page.locator('#input').fill('Write a durable file');await page.locator('#input').press('Enter');await expect(page.locator('#approval-panel')).toBeVisible();await page.locator('#approve').click();
        await expect(page.locator('#output')).toContainText('Wrote');
        if(!await page.locator('#file-panel').isVisible())await page.locator('#btn-files').click();
        await expect(page.locator('[data-path="/workspace"]')).toHaveAttribute('data-file-count','1');
    }finally{release();}
    await expect(page.locator('#status')).toHaveText('Ready');
    for(let i=0;i<3;i++){
        await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
        expect(await page.evaluate(async()=>{
            const root=await (await navigator.storage.getDirectory()).getDirectoryHandle('workspace');
            return (await (await (await root.getDirectoryHandle('nested')).getFileHandle('durable.txt')).getFile()).text();
        })).toBe('committed by WASM');
        await expect(page.locator('[data-path="/workspace"]')).toHaveAttribute('data-file-count','1');
    }
});
test('an older directory refresh cannot replace the latest file tree',async({page})=>{
    await ready(page);if(!await page.locator('#file-panel').isVisible())await page.locator('#btn-files').click();
    await page.evaluate(async()=>{
        const {BrowserAgent}=await import('/pkg/i.js');window.refreshCalls=[];
        BrowserAgent.prototype.list_files=function(){return new Promise(resolve=>window.refreshCalls.push(resolve));};
        const language=document.getElementById('lang-select');language.value='zh-CN';language.dispatchEvent(new Event('change'));
    });
    await expect.poll(()=>page.evaluate(()=>window.refreshCalls.length)).toBe(1);
    await page.evaluate(()=>{const language=document.getElementById('lang-select');language.value='en';language.dispatchEvent(new Event('change'));});
    await expect.poll(()=>page.evaluate(()=>window.refreshCalls.length)).toBe(2);
    await page.evaluate(()=>window.refreshCalls[1]([{path:'new.txt',size:1,directory:false}]));
    await expect(page.locator('#file-panel')).toContainText('new.txt');
    await page.evaluate(async()=>{window.refreshCalls[0]([{path:'old.txt',size:1,directory:false}]);await new Promise(resolve=>setTimeout(resolve,0));});
    await expect(page.locator('#file-panel')).toContainText('new.txt');await expect(page.locator('#file-panel')).not.toContainText('old.txt');
});
test('stopping a queued ZIP returns Ready before unlock and never downloads later',async({page,context})=>{
    await ready(page);const other=await context.newPage();await ready(other);let downloads=0;page.on('download',()=>downloads++);
    await other.evaluate(async()=>{const fs=await import('/platform.js');window.held=await fs.opfsBegin(await fs.opfsOpen(),'exclusive');});
    await page.locator('#btn-download').click();
    await expect.poll(()=>page.evaluate(async()=>{const {pending}=await navigator.locks.query();return pending.some(lock=>lock.name==='harness:files:default');})).toBe(true);
    await page.locator('#btn-cancel').click();
    try{await expect(page.locator('#status')).toHaveText('Ready');}
    finally{await other.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsRelease(window.held);});}
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWithTransaction(await fs.opfsOpen(),'exclusive',()=>{});});
    expect(downloads).toBe(0);await expect(page.locator('#downloads a')).toHaveCount(0);
});
test('a queued plugin write cannot mutate files after its invocation is cancelled',async({page})=>{
    await ready(page);
    const result=await page.evaluate(async()=>{
        const fs=await import('/platform.js'),{createPluginBroker}=await import('/plugins/broker.js'),root=await fs.opfsOpen();
        const held=await fs.opfsBegin(root,'exclusive'),abort=new AbortController(),effects={started:false};
        const descriptor={pluginId:'example.test',version:'1.0.0',digest:'a'.repeat(64),publicName:'p_test_0000000000000000',permissions:{workspace:'read-write',storage:false}};
        const context={workspaceId:'default',mode:'auto',signal:abort.signal,effects},broker=createPluginBroker({snapshot:{workspaceId:'default',tools:[descriptor]},descriptor,context,store:{}});
        const pending=broker({id:1,op:'fs.write',args:{path:'late-plugin',text:'cancelled'}});
        while(!(await navigator.locks.query()).pending.some(lock=>lock.name==='harness:files:default'))await new Promise(resolve=>setTimeout(resolve,5));
        abort.abort();await fs.opfsRelease(held);const outcome=await pending;return {ok:outcome.ok,effect:effects.started,file:await fs.opfsMetadata(root,'late-plugin')};
    });
    expect(result).toEqual({ok:false,effect:false,file:null});
});
test('stopping a queued UI upload prevents a late file creation',async({page,context})=>{
    await ready(page);const other=await context.newPage();await ready(other);
    await other.evaluate(async()=>{const fs=await import('/platform.js');window.held=await fs.opfsBegin(await fs.opfsOpen(),'exclusive');});
    await page.locator('#file-input').setInputFiles({name:'cancel-upload',mimeType:'text/plain',buffer:Buffer.from('not committed')});
    await expect.poll(()=>page.evaluate(async()=>{const {pending}=await navigator.locks.query();return pending.some(lock=>lock.name==='harness:files:default');})).toBe(true);
    await page.locator('#btn-cancel').click();
    try{await expect(page.locator('#status')).toHaveText('Ready');}
    finally{await other.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsRelease(window.held);});}
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return fs.opfsMetadata(await fs.opfsOpen(),'cancel-upload');})).toBeNull();
});
test('upload refuses to overwrite a file changed while its approval was pending',async({page,context})=>{
    await ready(page);await expect(page.locator('#save-status')).toHaveText('Saved');const other=await context.newPage();await ready(other);
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'upload-race.txt',new TextEncoder().encode('before'));});
    await page.locator('#file-input').setInputFiles({name:'upload-race.txt',mimeType:'text/plain',buffer:Buffer.from('mine')});
    await expect(page.locator('#approval-panel')).toBeVisible();
    await other.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'upload-race.txt',new TextEncoder().encode('external'));});
    await page.locator('#approve').click();await expect(page.locator('#status')).toHaveText('Ready');
    expect(await other.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'upload-race.txt',100));})).toBe('external');
    await expect(page.locator('#output')).toContainText('changed');
});
test('read scopes share across windows, block raw writers, and do not grant ambient reentrancy',async({page,context})=>{
    await ready(page);const other=await context.newPage();await ready(other);
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'state.txt',new TextEncoder().encode('old'));window.heldRoot=await fs.opfsBegin(await fs.opfsOpen(),'shared');});
    expect(await other.evaluate(async()=>{const fs=await import('/platform.js');return fs.opfsWithTransaction(await fs.opfsOpen(),'shared',async root=>new TextDecoder().decode(await fs.opfsRead(root,'state.txt',100)));})).toBe('old');
    await other.evaluate(()=>{window.writeDone=false;window.writePending=(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'state.txt',new TextEncoder().encode('new'));window.writeDone=true;})();});
    await expect.poll(()=>other.evaluate(async()=>{const locks=await navigator.locks.query();return locks.pending.some(lock=>lock.name==='harness:files:default');})).toBe(true);
    expect(await other.evaluate(()=>window.writeDone)).toBe(false);
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js?duplicate=1');return new TextDecoder().decode(await fs.opfsRead(window.heldRoot,'state.txt',100));})).toBe('old');
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsRelease(window.heldRoot);});await other.evaluate(()=>window.writePending);
    expect(await other.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'state.txt',100));})).toBe('new');
});
test('concurrent read-modify-write scopes from two windows do not lose updates',async({page,context})=>{
    await ready(page);const other=await context.newPage();await ready(other);
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'counter',new TextEncoder().encode('0'));});
    const increment=async page=>page.evaluate(async()=>{const fs=await import('/platform.js'),root=await fs.opfsOpen();for(let i=0;i<10;i++)await fs.opfsWithTransaction(root,'exclusive',async lease=>{const value=Number(new TextDecoder().decode(await fs.opfsRead(lease,'counter',100)));await fs.opfsWrite(lease,'counter',new TextEncoder().encode(String(value+1)));});});
    await Promise.all([increment(page),increment(other)]);
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'counter',100));})).toBe('20');
});
test('closing a writer window releases locks and preserves the previously committed file',async({page,context})=>{
    await ready(page);const writer=await context.newPage();await ready(writer);
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'durable',new TextEncoder().encode('committed'));});
    await writer.evaluate(async()=>{const fs=await import('/platform.js'),root=await fs.opfsOpen();window.lease=await fs.opfsBegin(root,'exclusive');window.writer=await (await root.getFileHandle('durable')).createWritable();await window.writer.write(new TextEncoder().encode('uncommitted'));});
    await writer.close();
    expect(await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'durable',100));})).toBe('committed');
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'durable',new TextEncoder().encode('next'));});
});
test('cancelling a queued transaction never performs a delayed write',async({page,context})=>{
    await ready(page);const other=await context.newPage();await ready(other);
    await page.evaluate(async()=>{const fs=await import('/platform.js');window.held=await fs.opfsBegin(await fs.opfsOpen(),'exclusive');});
    await other.evaluate(()=>{window.abortLock=new AbortController();window.lockOutcome=(async()=>{const fs=await import('/platform.js');try{await fs.opfsWithTransaction(await fs.opfsOpen(),'exclusive',root=>fs.opfsWrite(root,'cancelled',new Uint8Array([1])),window.abortLock.signal);return 'wrote';}catch(error){return error.name;}})();});
    await expect.poll(()=>other.evaluate(async()=>{const {pending}=await navigator.locks.query();return pending.some(lock=>lock.name==='harness:files:default');})).toBe(true);
    expect(await other.evaluate(async()=>{window.abortLock.abort();return window.lockOutcome;})).toBe('AbortError');
    await page.evaluate(async()=>{const fs=await import('/platform.js');await fs.opfsRelease(window.held);});
    expect(await other.evaluate(async()=>{const fs=await import('/platform.js');return fs.opfsMetadata(await fs.opfsOpen(),'cancelled');})).toBeNull();
});
test('a failed close aborts a real writable stream and preserves committed bytes',async({page})=>{
    await ready(page);
    const outcome=await page.evaluate(async()=>{
        const fs=await import('/platform.js'),root=await fs.opfsOpen(),encode=text=>new TextEncoder().encode(text);
        await fs.opfsWrite(root,'safe',encode('before'));
        const original=FileSystemFileHandle.prototype.createWritable;let message;
        FileSystemFileHandle.prototype.createWritable=async function(...args){const writer=await original.apply(this,args);return {write:writer.write.bind(writer),abort:writer.abort.bind(writer),close:async()=>{throw new DOMException('injected quota','QuotaExceededError');}};};
        try{await fs.opfsWrite(root,'safe',encode('after'));}catch(error){message=error.name;}finally{FileSystemFileHandle.prototype.createWritable=original;}
        const preserved=new TextDecoder().decode(await fs.opfsRead(root,'safe',100));await fs.opfsWrite(root,'safe',encode('next'));
        return {message,preserved,next:new TextDecoder().decode(await fs.opfsRead(root,'safe',100))};
    });
    expect(outcome).toEqual({message:'QuotaExceededError',preserved:'before',next:'next'});
});
test('a real WASM edit holds one lease across both reads and its commit',async({page,context})=>{
    await ready(page);const other=await context.newPage();await ready(other);
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.evaluate(async()=>{
        const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'race.txt',new TextEncoder().encode('a b'));
        const original=FileSystemFileHandle.prototype.getFile;let reads=0;
        FileSystemFileHandle.prototype.getFile=async function(...args){const file=await original.apply(this,args);if(this.name==='race.txt'&&++reads===2){window.editAtCommit=true;await new Promise(resolve=>window.resumeEdit=resolve);}return file;};
    });
    await page.route('**/mock/v1/chat/completions',route=>{
        const first=route.request().postDataJSON().messages.at(-1).role==='user';
        const delta=first?{tool_calls:[{index:0,id:'edit-race',type:'function',function:{name:'edit',arguments:JSON.stringify({path:'race.txt',edits:[{old:'a',new:'A'}]})}}]}:{content:'Edit complete'};
        return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:first?'tool_calls':'stop'}]})}\n\ndata: [DONE]\n\n`});
    });
    await page.locator('#input').fill('Edit race');await page.locator('#input').press('Enter');await expect(page.locator('#approval-panel')).toBeVisible();
    expect(await page.evaluate(async()=>{const {held}=await navigator.locks.query();return held.some(lock=>lock.name==='harness:files:default');})).toBe(false);
    await page.locator('#approve').click();await expect.poll(()=>page.evaluate(()=>window.editAtCommit)).toBe(true);
    await other.evaluate(()=>{window.competing=(async()=>{const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'race.txt',new TextEncoder().encode('external'));})();});
    await expect.poll(()=>other.evaluate(async()=>{const {pending}=await navigator.locks.query();return pending.some(lock=>lock.name==='harness:files:default');})).toBe(true);
    await page.evaluate(()=>window.resumeEdit());await other.evaluate(()=>window.competing);
    await expect(page.locator('#output')).toContainText('Edit complete');await expect(page.locator('#status')).toHaveText('Ready');
    expect(await other.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'race.txt',100));})).toBe('external');
});

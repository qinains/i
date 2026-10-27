import {test,expect} from './fixtures.mjs';
test.setTimeout(120_000);
async function configure(page){
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready',{timeout:30_000});await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#api-key').fill('ANALYSIS_FIXTURE_KEY');await page.locator('#stream').uncheck();await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();
}
const completion=text=>({choices:[{finish_reason:'stop',message:{role:'assistant',content:text}}]});
async function send(page,text){await page.locator('#input').fill(text);await page.locator('#input').press('Enter');}
async function savedSession(page){
    await expect(page.locator('#save-status')).toHaveText('Saved');
    const id=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    return page.evaluate(async id=>{const store=await import('/state/store.js');const db=await store.openStateStore();try{return (await db.load()).sessions.find(record=>record.id===id);}finally{db.close();}},id);
}
test('reflection cannot widen a bundled specification beyond its declared roles',async({page})=>{
    const bodies=[];
    await page.route('**/mock/v1/chat/completions',async route=>{bodies.push(route.request().postDataJSON());await route.fulfill({contentType:'application/json',body:JSON.stringify(completion('seed reply'))});});
    await page.route('**/reflection.json',route=>route.fulfill({contentType:'application/json',body:JSON.stringify({prompt:'Analyze only the declared user messages.',selection:{roles:['user'],start:0,end:null,max_messages:40,max_bytes:48000}})}));
    await configure(page);await send(page,'seed user message');await expect(page.locator('#output')).toContainText('seed reply');const before=await savedSession(page);
    page.once('dialog',dialog=>dialog.accept());await send(page,'/reflect assistant');await expect(page.locator('#output')).toContainText('Invalid analysis specification.');await expect(page.locator('#status')).toHaveText('Ready');
    expect(bodies).toHaveLength(1);await expect(page.locator('.line.analysis')).toHaveCount(0);expect((await savedSession(page)).history).toEqual(before.history);
});
test('explicit reflection persists only an independent report and one sanitized no-tools request',async({page})=>{
    const bodies=[];let report=false;
    await page.route('**/mock/v1/chat/completions',async route=>{const body=route.request().postDataJSON();bodies.push(body);await route.fulfill({contentType:'application/json',body:JSON.stringify(completion(report?'<img src=x onerror="window.analysisInjected=true"> verified lesson':'ordinary assistant seed'))});});
    await configure(page);expect(bodies).toHaveLength(0);await send(page,'original user seed');await expect(page.locator('#output')).toContainText('ordinary assistant seed');
    const before=await savedSession(page),title=await page.locator('#conversation-title').textContent();report=true;
    page.once('dialog',dialog=>dialog.accept());await send(page,'/reflect user');await expect(page.locator('.line.analysis')).toContainText('verified lesson');await expect(page.locator('#status')).toHaveText('Ready');
    expect(bodies).toHaveLength(2);const request=bodies[1],text=JSON.stringify(request);
    for(const key of ['tools','tool_choice','parallel_tool_calls','functions','function_call'])expect(request[key]).toBeUndefined();
    expect(text).toContain('original user seed');expect(text).not.toContain('ordinary assistant seed');expect(text).not.toContain('ANALYSIS_FIXTURE_KEY');
    expect(request.messages[0].content).not.toBe(bodies[0].messages[0].content);expect(await page.locator('#conversation-title').textContent()).toBe(title);
    await expect(page.locator('.line.analysis img')).toHaveCount(0);expect(await page.evaluate(()=>window.analysisInjected===true)).toBe(false);await expect(page.locator('#file-list')).toContainText('No files');
    const after=await savedSession(page);expect(after.history).toEqual(before.history);expect(after.name).toBe(before.name);expect(after.transcript.filter(line=>line.type==='analysis')).toHaveLength(1);expect(after.transcript.filter(line=>line.type==='user')).toEqual(before.transcript.filter(line=>line.type==='user'));
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('.line.analysis')).toContainText('verified lesson');expect(bodies).toHaveLength(2);
    report=false;await send(page,'continue normally');await expect.poll(()=>bodies.length).toBe(3);expect(JSON.stringify(bodies[2].messages)).not.toContain('verified lesson');
});
test('cancelled reflection discards late output without changing history',async({page})=>{
    let analyzing=false,release;const bodies=[];
    await page.route('**/mock/v1/chat/completions',async route=>{bodies.push(route.request().postDataJSON());if(analyzing)await new Promise(resolve=>{release=resolve;});await route.fulfill({contentType:'application/json',body:JSON.stringify(completion(analyzing?'late reflection':'seed reply'))}).catch(()=>{});});
    await configure(page);await send(page,'seed before cancellation');await expect(page.locator('#output')).toContainText('seed reply');const before=await savedSession(page);analyzing=true;
    page.once('dialog',dialog=>dialog.accept());await send(page,'/reflect');await expect.poll(()=>bodies.length).toBe(2);
    const unlocked=await page.evaluate(()=>navigator.locks.request('harness:workspace:default',{ifAvailable:true},lock=>!!lock));
    if(!unlocked)release?.();
    expect(unlocked).toBe(true);
    await page.locator('#btn-cancel').click();release?.();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('.line.analysis')).toHaveCount(0);
    expect((await savedSession(page)).history).toEqual(before.history);await page.reload();await expect(page.locator('#status')).toHaveText('Ready');expect(bodies).toHaveLength(2);
});
test('read-only reflection finishes while another file writer owns the OPFS lock',async({page})=>{
    let report=false;
    await page.route('**/mock/v1/chat/completions',route=>route.fulfill({contentType:'application/json',body:JSON.stringify(completion(report?'reflection without file access':'seed before lock'))}));
    await configure(page);await send(page,'seed');await expect(page.locator('#output')).toContainText('seed before lock');
    await page.evaluate(async()=>{
        let ready;const acquired=new Promise(resolve=>ready=resolve);
        window.analysisFileLock=navigator.locks.request('harness:files:default',async()=>{ready();await new Promise(resolve=>window.releaseAnalysisFileLock=resolve);});
        await acquired;
    });
    try{report=true;page.once('dialog',dialog=>dialog.accept());await send(page,'/reflect');await expect(page.locator('.line.analysis')).toContainText('reflection without file access');await expect(page.locator('#status')).toHaveText('Ready');}
    finally{await page.evaluate(async()=>{window.releaseAnalysisFileLock?.();await window.analysisFileLock;});}
});
test('reflection rejects model tool calls and never queues an analysis command',async({page})=>{
    let phase='seed',release;const bodies=[];
    await page.route('**/mock/v1/chat/completions',async route=>{bodies.push(route.request().postDataJSON());if(phase==='waiting')await new Promise(resolve=>{release=resolve;});const result=phase==='tool'?{choices:[{finish_reason:'tool_calls',message:{role:'assistant',content:null,tool_calls:[{id:'forbidden',type:'function',function:{name:'write',arguments:'{"path":"forbidden.txt","content":"bad"}'}}]}}]}:completion('seed ready');await route.fulfill({contentType:'application/json',body:JSON.stringify(result)}).catch(()=>{});});
    await configure(page);await send(page,'seed');await expect(page.locator('#output')).toContainText('seed ready');phase='tool';page.once('dialog',dialog=>dialog.accept());await send(page,'/reflect assistant');await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('.line.analysis')).toHaveCount(0);await expect(page.locator('#approval-panel')).toBeHidden();await expect(page.locator('#file-list')).toContainText('No files');
    const count=bodies.length;await send(page,'/reflect unsupported');await expect(page.locator('#global-error')).toContainText('Usage: /reflect');expect(bodies).toHaveLength(count);
    phase='waiting';await send(page,'normal message in progress');await expect.poll(()=>bodies.length).toBe(count+1);await send(page,'/reflect user');await expect(page.locator('#global-error')).toContainText('Finish or stop');expect(bodies).toHaveLength(count+1);expect((await savedSession(page)).pendingMessages??[]).toHaveLength(0);await page.locator('#input').fill('');await page.locator('#btn-cancel').click();release?.();await expect(page.locator('#status')).toHaveText('Ready');
});

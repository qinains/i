import {test, expect} from './fixtures.mjs';

test('closing BrowserAgent cancels pending approval without dispatch and rejects new work', async ({page}) => {
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
  await page.route('**/lifecycle-model/**', route => route.fulfill({contentType: 'application/json', body: JSON.stringify({choices: [{finish_reason: 'tool_calls', message: {role: 'assistant', content: null, tool_calls: [{id: 'close-call', type: 'function', function: {name: 'write', arguments: JSON.stringify({path: 'must-not-exist.txt', content: 'denied'})}}]}}]})}));
  const result = await page.evaluate(async () => {
    const {BrowserAgent} = await import('/pkg/i.js');
    const approval = Promise.withResolvers();
    const agent = await BrowserAgent.create_workspace('default', () => {}, () => { approval.resolve(); return new Promise(() => {}); }, () => {}, async () => {});
    agent.configure(JSON.stringify({base_url: `${location.origin}/lifecycle-model`, model: 'fixture', stream: false}));
    const running = agent.run('write a file').catch(error => String(error));
    await Promise.race([approval.promise, running.then(result => { throw new Error(`Run ended before approval: ${result}`); })]);
    await Promise.all([agent.close(), agent.close()]); await running;
    let rejected = false;
    try { agent.configure(JSON.stringify({base_url: `${location.origin}/lifecycle-model`, model: 'fixture'})); } catch (error) { rejected = /closed|closing/u.test(String(error)); }
    const fs = await import('/platform.js');
    const metadata = await fs.opfsMetadata(await fs.opfsOpen(), 'must-not-exist.txt');
    const history = JSON.parse(agent.export_history()); agent.free();
    return {rejected, metadata, retainedUser: history.some(item => item.role === 'user')};
  });
  expect(result).toEqual({rejected: true, metadata: null, retainedUser: true});
});

test('closing BrowserAgent waits for an in-flight checkpoint before releasing the adapter', async ({page}) => {
  await page.goto('/'); await expect(page.locator('#status')).toHaveText('Ready');
  const result = await page.evaluate(async () => {
    const {BrowserAgent} = await import('/pkg/i.js');
    const entered = Promise.withResolvers(), release = Promise.withResolvers();
    let checkpoints = 0;
    const agent = await BrowserAgent.create_workspace('default', () => {}, () => true, () => {}, async () => {
      if (++checkpoints === 1) { entered.resolve(); await release.promise; }
    });
    agent.configure(JSON.stringify({base_url: `${location.origin}/unused-model`, model: 'fixture', stream: false}));
    const running = agent.run('retain my message').catch(error => String(error));
    await entered.promise;
    let closed = false; const closing = agent.close().then(() => { closed = true; });
    await Promise.resolve(); const early = closed;
    release.resolve(); await running; await closing;
    const history = JSON.parse(agent.export_history()); agent.free();
    return {early, closed, retainedUser: history.some(item => item.role === 'user' && item.content === 'retain my message')};
  });
  expect(result).toEqual({early: false, closed: true, retainedUser: true});
});

test('the upstream application drains a real WASM checkpoint before closing persistent state',async({page})=>{
  await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
  const result=await page.evaluate(async()=>{
    const {BrowserAgent}=await import('/pkg/i.js'),{createBrowserApplication}=await import('/application.js');
    const entered=Promise.withResolvers(),release=Promise.withResolvers(),events=[],saved=[];
    const store={async commitSession(record,revision){entered.resolve();await release.promise;saved.push(record);events.push('checkpoint');return revision+1;},close(){events.push('store');}};
    const app=createBrowserApplication({Agent:BrowserAgent,store,hooks:{event(){},approve:()=>true,download(){}}});
    const record={id:'app-checkpoint',workspaceId:'default',revision:0,history:[{role:'system',content:'keep'}],connectionId:'a',connectionIdentity:''};
    const agent=await app.controller.configure(record,{base_url:`${location.origin}/unused-model`,model:'fixture',stream:false},'a');
    const running=agent.run('retain this message').catch(error=>String(error));await entered.promise;
    const closing=app.shutdown();await Promise.resolve();const early=events.includes('store');
    release.resolve();await running;const report=await closing;
    return {early,closed:report.closed,last:events.at(-1),retained:saved.some(record=>record.history.some(item=>item.role==='user'&&item.content==='retain this message'))};
  });
  expect(result).toEqual({early:false,closed:true,last:'store',retained:true});
});

test('BFCache suspension cancels work without terminally closing the application',async({page})=>{
  await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
  await page.locator('#settings-btn').click();await page.locator('#base-url').fill(`${new URL(page.url()).origin}/mock/v1`);
  await page.locator('#model').fill('fixture');await page.locator('#stream').uncheck();await page.locator('#save-config').click();await page.locator('#close-settings').click();
  await page.route('**/mock/v1/chat/completions',route=>route.fulfill({contentType:'application/json',body:JSON.stringify({choices:[{finish_reason:'stop',message:{role:'assistant',content:'Still usable after suspension.'}}]})}));
  await page.evaluate(()=>window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:true})));
  await page.locator('#input').fill('still usable');await page.locator('#input').press('Enter');
  await expect(page.locator('#output')).toContainText('Still usable after suspension.');
});

test('Ready is not published while the initial real adapter is still being acquired',async({page})=>{
  await page.route('**/pkg/i.js',async route=>{
    const response=await route.fetch();
    const body=await response.text()+`
const originalWorkspaceFactory=BrowserAgent.create_workspace;
BrowserAgent.create_workspace=async function(...args){
    window.initialAdapterEntered=true;
    await new Promise(resolve=>{window.releaseInitialAdapter=resolve;});
    return originalWorkspaceFactory.apply(this,args);
};`;
    await route.fulfill({response,body});
  });
  await page.goto('/');await page.waitForFunction(()=>window.initialAdapterEntered);
  await expect(page.locator('#status')).not.toHaveText('Ready');
  await page.evaluate(()=>window.releaseInitialAdapter());
  await expect(page.locator('#status')).toHaveText('Ready');
  await expect(page.locator('#save-status')).toHaveText('Saved');
});

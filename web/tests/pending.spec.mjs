import {test,expect} from './fixtures.mjs';
const endpoint='**/mock/v1/chat/completions';
const reply=(content='Done',reason='stop')=>`data: ${JSON.stringify({choices:[{index:0,delta:{content},finish_reason:reason}]})}\n\ndata: [DONE]\n\n`;
async function configure(page){
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#close-settings').click();
}
async function send(page,text){await page.locator('#input').fill(text);await page.locator('#btn-send').click();}
test('replying swaps Stop with Queue only for a nonempty draft and restores Stop after queueing',async({page})=>{
    await configure(page);
    await page.evaluate(()=>{const original=window.fetch;window.fetch=(url,options)=>String(url).includes('/mock/v1/chat/completions')?new Promise((resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new DOMException('Aborted','AbortError')),{once:true})):original(url,options);});
    await send(page,'first');await expect(page.locator('#status')).toHaveText('Processing...');await expect(page.locator('#input')).toBeEnabled();
    await expect(page.locator('#btn-cancel')).toBeVisible();await expect(page.locator('#btn-send')).toBeHidden();
    const stop=await page.locator('#btn-cancel').boundingBox();
    await page.locator('#input').fill('   ');await expect(page.locator('#btn-cancel')).toBeVisible();await expect(page.locator('#btn-send')).toBeHidden();
    await page.locator('#input').fill('follow-up');await expect(page.locator('#btn-cancel')).toBeHidden();await expect(page.locator('#btn-send')).toHaveText('Queue message');await expect(page.locator('#btn-send')).toBeVisible();
    const queue=await page.locator('#btn-send').boundingBox();expect(queue.x+queue.width).toBeCloseTo(stop.x+stop.width,0);
    await page.locator('#input').fill('');await expect(page.locator('#btn-cancel')).toBeVisible();await expect(page.locator('#btn-send')).toBeHidden();
    await send(page,'follow-up');await expect(page.locator('.pending-message')).toContainText('follow-up');await expect(page.locator('#input')).toHaveValue('');
    await expect(page.locator('#btn-send')).toBeHidden();await expect(page.locator('#btn-cancel')).toBeVisible();
    await page.locator('#btn-cancel').click();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#btn-cancel')).toBeHidden();await expect(page.locator('#btn-send')).toBeVisible();
});
test('queued follow-ups can be edited and cancelled before ordered dispatch',async({page})=>{
    await configure(page);const bodies=[];let release;const gate=new Promise(resolve=>release=resolve);
    await page.route(endpoint,async route=>{bodies.push(route.request().postDataJSON());if(bodies.length===1)await gate;await route.fulfill({contentType:'text/event-stream',body:reply()});});
    await send(page,'first');await expect(page.locator('#status')).toHaveText('Processing...');await expect(page.locator('#input')).toBeEnabled();
    await send(page,'second');await send(page,'cancel this');
    await expect(page.locator('.pending-message')).toHaveCount(2);expect(bodies).toHaveLength(1);
    await page.locator('.pending-message').first().getByRole('button',{name:'Edit',exact:true}).click();
    await page.locator('.pending-message textarea').fill('second edited');await page.locator('.pending-message').first().getByRole('button',{name:'Save',exact:true}).click();
    await page.locator('.pending-message').last().getByRole('button',{name:'Cancel message',exact:true}).click();
    await page.locator('#queue-resume').click();release();
    await expect(page.locator('#pending-messages')).toBeHidden();await expect(page.locator('#status')).toHaveText('Ready');
    expect(bodies).toHaveLength(2);expect(bodies[1].messages.at(-1).content).toBe('second edited');
    expect(JSON.stringify(bodies)).not.toContain('cancel this');
});
test('failed request pauses queued images; reload never sends them without confirmation',async({page})=>{
    await configure(page);const bodies=[];let release;const gate=new Promise(resolve=>release=resolve);
    await page.route(endpoint,async route=>{bodies.push(route.request().postDataJSON());if(bodies.length===1)await gate;await route.fulfill({contentType:'text/event-stream',body:reply('Done',bodies.length===1?'length':'stop')});});
    await send(page,'first');await expect(page.locator('#input')).toBeEnabled();
    const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=';
    await page.locator('#input').evaluate((input,png)=>{
        const data=new DataTransfer();data.items.add(new File([Uint8Array.from(atob(png),c=>c.charCodeAt(0))],'image.png',{type:'image/png'}));
        // Firefox ignores clipboardData in the synthetic ClipboardEvent constructor.
        const event=new ClipboardEvent('paste',{bubbles:true,cancelable:true});Object.defineProperty(event,'clipboardData',{value:data});input.dispatchEvent(event);
    },png);
    await expect(page.locator('#input')).toHaveValue('[Image #1]');await page.locator('#input').fill('[Image #1] queued caption');await page.locator('#btn-send').click();
    release();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#queue-resume')).toBeVisible();
    await expect(page.locator('#save-status')).toHaveText('Saved');await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('.pending-message')).toHaveCount(1);expect(bodies).toHaveLength(1);
    await page.locator('#input').fill('Do not clear this draft');await page.locator('#queue-resume').click();
    await expect(page.locator('#pending-messages')).toBeHidden();await expect(page.locator('#status')).toHaveText('Ready');
    expect(bodies).toHaveLength(2);expect(bodies[1].messages.at(-1).content).toEqual([{type:'text',text:'queued caption'},{type:'image_url',image_url:{url:`data:image/png;base64,${png}`}}]);
    await expect(page.locator('#input')).toHaveValue('Do not clear this draft');
});
test('stopping a task pauses its queue and switching tabs never moves queued messages',async({page})=>{
    await configure(page);const first=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    await page.locator('#btn-new-session').click();await expect(page.locator('[role=tab][aria-selected=true]')).not.toHaveAttribute('data-session-id',first);
    const second=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');await page.locator(`[data-session-id="${first}"]`).click();
    const bodies=[];await page.route(endpoint,route=>{bodies.push(route.request().postDataJSON());return route.abort('aborted');});
    await page.evaluate(()=>{const original=window.fetch;window.fetch=(url,options)=>String(url).includes('/mock/v1/chat/completions')?new Promise((resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new DOMException('Aborted','AbortError')),{once:true})):original(url,options);});
    await send(page,'first');await send(page,'owned follow-up');await page.locator('#btn-cancel').click();
    await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#queue-resume')).toBeVisible();
    await page.locator(`[data-session-id="${second}"]`).click();await expect(page.locator('#pending-messages')).toBeHidden();
    await page.locator(`[data-session-id="${first}"]`).click();await expect(page.locator('.pending-message')).toContainText('owned follow-up');expect(bodies).toHaveLength(0);
});
test('another follow-up can be queued while a queued message is running',async({page})=>{
    await configure(page);const bodies=[],releases=[];
    await page.route(endpoint,async route=>{bodies.push(route.request().postDataJSON());if(bodies.length<=2)await new Promise(resolve=>releases.push(resolve));await route.fulfill({contentType:'text/event-stream',body:reply()});});
    await send(page,'first');await expect.poll(()=>bodies.length).toBe(1);await send(page,'second');
    releases[0]();await expect.poll(()=>bodies.length).toBe(2);
    await expect(page.locator('#input')).toBeEnabled();await send(page,'third');releases[1]();
    await expect(page.locator('#pending-messages')).toBeHidden();await expect(page.locator('#status')).toHaveText('Ready');
    expect(bodies.map(body=>body.messages.at(-1).content)).toEqual(['first','second','third']);
});

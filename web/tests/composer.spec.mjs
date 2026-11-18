import {test,expect} from './fixtures.mjs';
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=';
async function paste(page,{image=false,text=''}={}){
    await page.locator('#input').evaluate((input,{image,text,png})=>{
        const data=new DataTransfer();if(image)data.items.add(new File([Uint8Array.from(atob(png),c=>c.charCodeAt(0))],'clipboard.png',{type:'image/png'}));else data.setData('text/plain',text);
        input.dispatchEvent(new ClipboardEvent('paste',{clipboardData:data,bubbles:true,cancelable:true}));
    },{image,text,png});
}
for(const protocol of ['openai','anthropic'])for(const caption of ['', 'Describe this image'])test(`composer ${protocol} pasted image waits and sends with ${caption?'caption':'no caption'}`,async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();
    await page.locator('#protocol').selectOption(protocol);await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
    const bodies=[];await page.route('**/mock/v1/**',route=>{
        bodies.push(route.request().postDataJSON());
        const body=protocol==='openai'?`data: ${JSON.stringify({choices:[{index:0,delta:{role:'assistant',content:'Image received.'},finish_reason:'stop'}]})}\n\ndata: [DONE]\n\n`:
            [['message_start',{message:{type:'message',role:'assistant',content:[],stop_reason:null}}],['content_block_start',{index:0,content_block:{type:'text',text:''}}],['content_block_delta',{index:0,delta:{type:'text_delta',text:'Image received.'}}],['content_block_stop',{index:0}],['message_delta',{delta:{stop_reason:'end_turn'}}],['message_stop',{}]].map(([type,data])=>`event: ${type}\ndata: ${JSON.stringify({type,...data})}\n\n`).join('');
        return route.fulfill({contentType:'text/event-stream',body});
    });
    await paste(page,{image:true});await expect(page.locator('#input')).toHaveValue('[Image #1]');await expect(page.locator('#composer-attachments img')).toHaveCount(1);expect(bodies).toHaveLength(0);
    await expect(page.locator('#file-list')).toContainText('No files');
    // Draft attachments survive reload and remain owned by this conversation.
    await expect(page.locator('#save-status')).toHaveText('Saved');await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#composer-attachments img')).toHaveCount(1);
    await page.locator('#settings-btn').click();await page.locator('#protocol').selectOption(protocol);await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    if(caption)await page.locator('#input').fill(`[Image #1] ${caption}`);
    await page.locator('#btn-send').click();await expect(page.locator('#output')).toContainText('Image received.');expect(bodies).toHaveLength(1);
    const content=bodies[0].messages.at(-1).content;
    if(protocol==='openai')expect(content.find(b=>b.type==='image_url').image_url.url).toBe(`data:image/png;base64,${png}`);
    else expect(content.find(b=>b.type==='image').source.data).toBe(png);
    expect(content.filter(b=>b.type==='text').map(b=>b.text).join('')).toBe(caption);await expect(page.locator('#composer-attachments')).toBeHidden();
});
test('long paste is folded, removable and safe',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const text='<script>untrusted</script>\n'.repeat(100);await paste(page,{text});await expect(page.locator('#input')).toHaveValue(/\[Pasted Content/);await expect(page.locator('#composer-attachments script')).toHaveCount(0);
    await page.locator('#input').press('End');await page.locator('#input').pressSequentially(' explain');
    await expect(page.locator('#save-status')).toHaveText('Saved');
    const draft=await page.evaluate(async()=>{const {openStateStore}=await import('/state/store.js');const s=await openStateStore(),data=await s.load();s.close();return data.sessions.find(r=>r.draft.includes('Pasted Content'));});
    expect(draft?.composerBlocks[0].text).toBe(text);
    await page.locator('#composer-attachments button').click();await expect(page.locator('#input')).toHaveValue(' explain');await expect(page.locator('#composer-attachments')).toBeHidden();
});
test('folded pasted content is sent in full with its added instruction',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    let sent;await page.route('**/mock/v1/chat/completions',route=>{
        sent=route.request().postDataJSON().messages.at(-1).content;
        return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta:{role:'assistant',content:'Text received.'},finish_reason:'stop'}]})}\n\ndata: [DONE]\n\n`});
    });
    const text='  preserve indentation\n'.repeat(100);await paste(page,{text});
    const marker=await page.locator('#input').inputValue();await page.locator('#input').fill(`${marker} explain`);await page.locator('#btn-send').click();
    await expect(page.locator('#output')).toContainText('Text received.');expect(sent).toBe(`${text} explain`);
    await expect(page.locator('.line.user')).toContainText('Pasted Content');await expect(page.locator('.line.user')).not.toContainText('preserve indentation');
});
test('sending images in one conversation never clears another conversation draft',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const first=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    await page.locator('#btn-new-session').click();await expect(page.locator('[role=tab][aria-selected=true]')).not.toHaveAttribute('data-session-id',first);
    const second=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    await page.locator(`[data-session-id="${first}"]`).click();
    await expect(page.locator('[role=tab][aria-selected=true]')).toHaveAttribute('data-session-id',first);
    await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.route('**/mock/v1/chat/completions',route=>route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta:{role:'assistant',content:'Image done.'},finish_reason:'stop'}]})}\n\ndata: [DONE]\n\n`}));
    await paste(page,{image:true});await expect(page.locator('#input')).toHaveValue('[Image #1]');
    await page.evaluate(()=>{const original=Blob.prototype.arrayBuffer;Blob.prototype.arrayBuffer=function(){if(this.size>12&&this.type==='image/png'&&!window.imageReleased)return new Promise(resolve=>{window.imageWaiting=true;window.releaseImage=async()=>{window.imageReleased=true;resolve(await original.call(this));};});return original.call(this);};});
    await page.locator('#btn-send').click();await expect.poll(()=>page.evaluate(()=>window.imageWaiting)).toBe(true);
    await page.locator(`[data-session-id="${second}"]`).click();await expect(page.locator('[role=tab][aria-selected=true]')).toHaveAttribute('data-session-id',second);await page.locator('#input').fill('Keep the second draft');
    await page.evaluate(()=>window.releaseImage());await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#input')).toHaveValue('Keep the second draft');
    await page.locator(`[data-session-id="${first}"]`).click();await page.locator(`[data-session-id="${second}"]`).click();
    await expect(page.locator('#input')).toHaveValue('Keep the second draft');
});
test('failed asynchronous image paste does not discard newly typed text',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#input').fill('before');
    await page.evaluate(()=>{
        const original=Blob.prototype.arrayBuffer;Blob.prototype.arrayBuffer=function(){if(this.size===3)return new Promise(resolve=>{window.releaseBadImage=async()=>resolve(await original.call(this));});return original.call(this);};
        const data=new DataTransfer();data.items.add(new File(['bad'],'bad.png',{type:'image/png'}));
        document.querySelector('#input').dispatchEvent(new ClipboardEvent('paste',{clipboardData:data,bubbles:true,cancelable:true}));
    });
    await expect.poll(()=>page.evaluate(()=>typeof window.releaseBadImage)).toBe('function');
    await page.locator('#input').fill('before + newly typed');await page.evaluate(()=>window.releaseBadImage());
    await expect(page.locator('#global-error')).toContainText('MIME');await expect(page.locator('#input')).toHaveValue('before + newly typed');
});

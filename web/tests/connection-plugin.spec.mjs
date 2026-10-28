import {test,expect as baseExpect} from './fixtures.mjs';
// Persistent macOS WebKit profiles can spend tens of seconds starting OPFS.
test.setTimeout(120_000);
const expect=baseExpect.configure({timeout:30_000});
const helper='http://127.0.0.1:43127',token='PAIRING_CAPABILITY_'.repeat(3);
const fragment='#connection-plugin='+Buffer.from(JSON.stringify({url:helper,token})).toString('base64url');
const existing={base_url:'https://api.example/v1/responses',model:'original',api_key:'EXISTING_API_KEY',protocol:'responses',max_tokens:65536,use_max_tokens:true,reasoning_effort:'default',language:'en',transport:'direct',stream:true,relay_token:'',custom_headers:''};
async function mockPlugin(page,{pendingLogin=false,badManifest=false,protocol='responses',retained=false}={}){
    const requests=[];let loggedIn=!retained,active='first',releaseLogin;
    await page.route(helper+'/**',async route=>{
        const request=route.request(),path=new URL(request.url()).pathname;
        if(request.method()==='OPTIONS'){await route.fulfill({status:204,headers:{'access-control-allow-origin':new URL(page.url()).origin,'access-control-allow-methods':'GET,POST,OPTIONS','access-control-allow-headers':'content-type,x-i-local-token'}});return;}
        requests.push({path,headers:request.headers(),body:request.postData()?request.postDataJSON():null});
        expect(request.headers()['x-i-local-token']).toBe(token);expect(request.headers().authorization).toBeUndefined();
        const headers={'access-control-allow-origin':new URL(page.url()).origin};
        let body={};
        if(path==='/v1/manifest')body={format_version:1,kind:'connection',id:protocol==='anthropic'?'anthropic.console':'openai.chatgpt',name:protocol==='anthropic'?'Claude Console':'ChatGPT',protocol:badManifest?'openai':protocol};
        if(path==='/v1/status')body={active:loggedIn?active:null,accounts:loggedIn||retained?[{id:'first',label:'First',email:'first@example.test',active:loggedIn&&active==='first',signed_in:loggedIn},{id:'second',label:'Second',email:'second@example.test',active:loggedIn&&active==='second',signed_in:loggedIn}]:[]};
        if(path==='/v1/models')body={models:[{slug:'real-model',display_name:'Real model',visibility:'list'},{slug:'hidden-model',display_name:'Hidden',visibility:'hide'}]};
        if(path==='/v1/select')active=request.postDataJSON().account;
        if(path==='/v1/logout')loggedIn=false;
        if(path==='/v1/login'&&pendingLogin)await new Promise(resolve=>{releaseLogin=resolve;});
        if(path==='/v1/login')loggedIn=true;
        if(path==='/v1/cancel')releaseLogin?.();
        if(path==='/v1/responses'){
            const response={id:'plugin_response',object:'response',status:'completed',output:[]};
            await route.fulfill({headers:{...headers,'content-type':'text/event-stream'},body:['response.created','response.completed'].map(type=>'data: '+JSON.stringify({type,response:{...response,status:type==='response.created'?'in_progress':'completed'}})+'\n\n').join('')});return;
        }
        if(path==='/v1/messages'){
            const events=[{type:'message_start',message:{id:'msg_local',type:'message',role:'assistant',content:[],stop_reason:null,usage:{input_tokens:1,output_tokens:0}}},{type:'content_block_start',index:0,content_block:{type:'text',text:''}},{type:'content_block_delta',index:0,delta:{type:'text_delta',text:'Claude plugin connected.'}},{type:'content_block_stop',index:0},{type:'message_delta',delta:{stop_reason:'end_turn'},usage:{output_tokens:2}},{type:'message_stop'}];
            await route.fulfill({headers:{...headers,'content-type':'text/event-stream'},body:events.map(event=>'event: '+event.type+'\ndata: '+JSON.stringify(event)+'\n\n').join('')});return;
        }
        await route.fulfill({headers:{...headers,'content-type':'application/json'},body:JSON.stringify(body)});
    });
    return requests;
}
test('pairing is an unsaved new plugin connection and inference uses only local capability',async({page})=>{
    await page.addInitScript(config=>{if(!localStorage.getItem('i_harness_profiles'))localStorage.setItem('i_harness_profiles',JSON.stringify({profiles:{original:config},current:'original'}));},existing);
    const requests=await mockPlugin(page);
    await page.goto('/'+fragment);await expect(page.locator('#status')).toHaveText('Ready');
    expect(new URL(page.url()).hash).toBe('');expect(requests).toHaveLength(0);
    await expect(page.locator('#transport')).toHaveValue('plugin');await expect(page.locator('#base-url')).toBeHidden();await expect(page.locator('#api-key')).toBeHidden();await expect(page.locator('#connection-guide')).toBeHidden();
    await expect(page.locator('#local-plugin-command')).toHaveText(/^i plugins serve chatgpt --origin /);
    expect(await page.evaluate(()=>JSON.parse(localStorage.getItem('i_harness_profiles')).current)).toBe('original');
    await page.locator('#local-plugin-check').click();await expect(page.locator('#local-plugin-status')).toHaveText('Plugin accounts and models refreshed.');
    await expect(page.locator('#local-plugin-identity')).toHaveText('ChatGPT (openai.chatgpt)');await expect(page.locator('#local-plugin-model')).toHaveValue('real-model');await expect(page.locator('#local-plugin-model option[value=hidden-model]')).toHaveCount(0);
    await page.locator('#remember-config').check();await page.locator('#save-config').click();
    const profiles=await page.evaluate(()=>JSON.parse(localStorage.getItem('i_harness_profiles')));
    expect(profiles.profiles.original).toEqual(existing);expect(Object.keys(profiles.profiles)).toHaveLength(2);expect(profiles.current).not.toBe('original');expect(profiles.profiles[profiles.current].transport).toBe('plugin');
    await page.locator('#test-connection').click();await expect(page.locator('#test-result')).toHaveText('Connection verified.');
    const inference=requests.find(item=>item.path==='/v1/responses');expect(inference.body.stream).toBe(true);expect(inference.body.model).toBe('real-model');expect(JSON.stringify(inference.body)).not.toContain(token);
    const count=requests.length;await page.reload();await expect(page.locator('#status')).toHaveText('Ready');expect(requests).toHaveLength(count);await page.locator('#settings-btn').click();await expect(page.locator('#transport')).toHaveValue('plugin');
    await page.locator('#local-plugin-check').click();await expect(page.locator('#local-plugin-account')).toHaveValue('first');page.once('dialog',dialog=>dialog.accept());await page.locator('#local-plugin-account').selectOption('second');await expect(page.locator('#local-plugin-status')).toHaveText('Plugin account selected.');await page.locator('#local-plugin-logout').click();await expect(page.locator('#local-plugin-status')).toHaveText('Plugin account signed out.');
});
test('plugin login is explicit and cancellation reaches native plugin before abort',async({page})=>{
    const requests=await mockPlugin(page,{pendingLogin:true});await page.goto('/'+fragment);await expect(page.locator('#status')).toHaveText('Ready');expect(requests).toHaveLength(0);
    await page.locator('#local-plugin-login').click();await expect(page.locator('#local-plugin-status')).toHaveText('Waiting for provider login…');await expect.poll(()=>requests.some(item=>item.path==='/v1/login')).toBe(true);
    await page.locator('#local-plugin-cancel').click();await expect.poll(()=>requests.some(item=>item.path==='/v1/cancel')).toBe(true);await expect(page.locator('#local-plugin-cancel')).toBeHidden();
});
test('retained provider registrations can be reauthorized and adding a new account is explicit',async({page})=>{
    const requests=await mockPlugin(page,{retained:true});await page.goto('/'+fragment);await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#local-plugin-login').click();await expect(page.locator('#local-plugin-status')).toHaveText('Plugin accounts and models refreshed.');
    expect(requests.find(request=>request.path==='/v1/login').body).toEqual({account:'first'});
    page.once('dialog',dialog=>dialog.dismiss());await page.locator('#local-plugin-account').selectOption('second');
    await expect(page.locator('#local-plugin-account')).toHaveValue('first');expect(requests.some(request=>request.path==='/v1/select')).toBe(false);
    await page.locator('#local-plugin-account').selectOption('__new__');page.once('dialog',dialog=>dialog.dismiss());await page.locator('#local-plugin-login').click();
    await expect(page.locator('#local-plugin-account')).toHaveValue('first');expect(requests.filter(request=>request.path==='/v1/login')).toHaveLength(1);
    await page.locator('#local-plugin-account').selectOption('__new__');page.once('dialog',dialog=>dialog.accept());await page.locator('#local-plugin-login').click();
    await expect.poll(()=>requests.filter(request=>request.path==='/v1/login').length).toBe(2);
    await expect(page.locator('#local-plugin-status')).toHaveText('Plugin accounts and models refreshed.');
    expect(requests.filter(request=>request.path==='/v1/login').at(-1).body).toEqual({});
});
test('invalid plugin manifest blocks all provider operations and unsafe pairing is stripped',async({page})=>{
    const requests=await mockPlugin(page,{badManifest:true});await page.goto('/'+fragment);await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#local-plugin-login').click();await expect(page.locator('#local-plugin-status')).toHaveText('Local helper request failed.');expect(requests.map(item=>item.path)).toEqual(['/v1/manifest']);
    const unsafe='#connection-plugin='+Buffer.from(JSON.stringify({url:'http://127.1:43127',token})).toString('base64url');await page.goto('/'+unsafe);await expect(page.locator('#status')).toHaveText('Ready');expect(new URL(page.url()).hash).toBe('');await expect(page.locator('#global-error')).toContainText('Pairing link is invalid.');expect(requests).toHaveLength(1);
});
test('Anthropic plugin manifest selects native Messages SSE without exposing provider authentication',async({page})=>{
    const requests=await mockPlugin(page,{protocol:'anthropic'});await page.goto('/'+fragment);await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#local-plugin-check').click();await expect(page.locator('#local-plugin-status')).toHaveText('Plugin accounts and models refreshed.');
    await expect(page.locator('#protocol')).toHaveValue('anthropic');await expect(page.locator('#base-url')).toHaveValue(helper+'/v1/messages');await expect(page.locator('#local-plugin-identity')).toHaveText('Claude Console (anthropic.console)');
    await page.locator('#remember-config').check();await page.locator('#save-config').click();await page.locator('#test-connection').click();await expect(page.locator('#test-result')).toHaveText('Connection verified.');
    const request=requests.find(item=>item.path==='/v1/messages');expect(request.body.stream).toBe(true);expect(request.body.messages).toBeDefined();
    for(const header of ['authorization','x-api-key','anthropic-version','anthropic-dangerous-direct-browser-access'])expect(request.headers[header]).toBeUndefined();
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#settings-btn').click();await expect(page.locator('#protocol')).toHaveValue('anthropic');
});
test('Claude same-profile reauthorization cannot change the recipient of existing history without confirmation',async({page})=>{
    const requests=await mockPlugin(page,{protocol:'anthropic'});
    await page.goto('/'+fragment);await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#local-plugin-check').click();await expect(page.locator('#local-plugin-account')).toHaveValue('first');
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('Private history for the original Console workspace');await page.locator('#btn-send').click();
    await expect(page.locator('#output')).toContainText('Claude plugin connected.');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('#local-plugin-check').click();
    await expect(page.locator('#local-plugin-account')).toHaveValue('first');
    let confirmations=0;
    const reject=async dialog=>{confirmations++;expect(dialog.message()).toContain('different recipient');await dialog.dismiss();};
    page.on('dialog',reject);await page.locator('#local-plugin-login').click();
    await expect(page.locator('#local-plugin-login')).toBeEnabled();
    expect(confirmations).toBe(1);expect(requests.filter(item=>item.path==='/v1/login')).toHaveLength(0);
    page.off('dialog',reject);page.once('dialog',dialog=>dialog.accept());await page.locator('#local-plugin-login').click();
    await expect.poll(()=>requests.filter(item=>item.path==='/v1/login').length).toBe(1);
    await expect(page.locator('#local-plugin-account')).toHaveValue('first');
    expect(requests.filter(item=>item.path==='/v1/messages')).toHaveLength(1);
});

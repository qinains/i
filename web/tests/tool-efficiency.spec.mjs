import {test,expect} from './fixtures.mjs';

for(const protocol of ['openai','anthropic'])test(`efficient ${protocol} tools discover, batch read and edit in four model requests`,async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await page.locator('#settings-btn').click();await page.locator('#protocol').selectOption(protocol);
    await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');
    await page.locator('#save-config').click();await page.locator('#close-settings').click();
    const filler='// irrelevant implementation detail\n'.repeat(500);
    for(const [name,text] of [['app.js',`${filler}function target() { return 'old'; }\nconst label = 'old label';\n`],['test.js',`${filler}assert(target() === 'old');\n`]]) {
        await page.locator('#file-input').setInputFiles({name,mimeType:'text/javascript',buffer:Buffer.from(text)});
        await expect(page.locator(`[data-file="${name}"]`)).toHaveCount(1);
    }
    const batches=[[
        ['glob',{patterns:['**/*.js']}],['search',{pattern:'target',glob:'**/*.js'}]
    ],[
        ['read',{files:[{path:'app.js',start_line:501,end_line:502},{path:'test.js',start_line:501,end_line:501}]}]
    ],[
        ['edit',{path:'app.js',edits:[{old:"return 'old'",new:"return 'new'"},{old:"'old label'",new:"'new label'"}]}]
    ]];
    const bodies=[];
    await page.route('**/mock/v1/**',route=>{
        const body=route.request().postDataJSON(),index=bodies.length;bodies.push(body);
        const calls=(batches[index]??[]).map(([name,args],i)=>({id:`call-${index}-${i}`,name,args}));
        if(protocol==='openai') {
            const delta={role:'assistant',content:calls.length?null:'Efficient update done.',...(calls.length?{tool_calls:calls.map((c,index)=>({index,id:c.id,type:'function',function:{name:c.name,arguments:JSON.stringify(c.args)}}))}:{})};
            return route.fulfill({contentType:'text/event-stream',body:`data: ${JSON.stringify({choices:[{index:0,delta,finish_reason:calls.length?'tool_calls':'stop'}]})}\n\ndata: [DONE]\n\n`});
        }
        const blocks=calls.length?calls.map(c=>({type:'tool_use',id:c.id,name:c.name,input:c.args})):[{type:'text',text:'Efficient update done.'}];
        const events=[['message_start',{message:{type:'message',role:'assistant',content:[],stop_reason:null}}]];
        blocks.forEach((b,index)=>{
            events.push(['content_block_start',{index,content_block:b.type==='tool_use'?{...b,input:{}}:{type:'text',text:''}}],
                ['content_block_delta',{index,delta:b.type==='tool_use'?{type:'input_json_delta',partial_json:JSON.stringify(b.input)}:{type:'text_delta',text:b.text}}],['content_block_stop',{index}]);
        });
        events.push(['message_delta',{delta:{stop_reason:calls.length?'tool_use':'end_turn'}}],['message_stop',{}]);
        return route.fulfill({contentType:'text/event-stream',body:events.map(([type,data])=>`event: ${type}\ndata: ${JSON.stringify({type,...data})}\n\n`).join('')});
    });
    await page.locator('#input').fill('Locate target and update both app strings without dumping files');await page.locator('#input').press('Enter');
    await expect(page.locator('#approval-panel')).toBeVisible();await expect(page.locator('#approval-description')).toContainText('TOOL edit');await page.locator('#approve').click();
    await expect(page.locator('#output')).toContainText('Efficient update done.');await expect(page.locator('#status')).toHaveText('Ready');
    expect(bodies).toHaveLength(4);
    const tools=protocol==='openai'?bodies[3].messages.filter(m=>m.role==='tool').map(m=>m.content):bodies[3].messages.flatMap(m=>Array.isArray(m.content)?m.content:[]).filter(b=>b.type==='tool_result').map(b=>Array.isArray(b.content)?b.content.map(c=>c.text??'').join(''):b.content);
    expect(tools).toHaveLength(4);expect(tools.every(t=>!t.startsWith('ERROR'))).toBe(true);
    expect(Buffer.byteLength(tools.join(''))).toBeLessThan(2000);
    const schema=bodies[0].tools.find(t=>(t.function?.name??t.name)==='read');
    expect((schema.function?.parameters??schema.input_schema).required).toEqual(['files']);
    const text=await page.evaluate(async()=>{const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'app.js',100000));});
    expect(text).toContain("return 'new'");expect(text).toContain("'new label'");
    await page.reload();await expect(page.locator('[data-file="app.js"]')).toHaveCount(1);expect(bodies).toHaveLength(4);
});

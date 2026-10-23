import {test,expect} from './fixtures.mjs';
test('plugin storage commits revisions and enforces atomic private KV quotas',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const result=await page.evaluate(async()=>{
        const {openPluginStore}=await import('/plugins/store.js');const s=await openPluginStore();
        const first=await s.loadRegistry();await s.commitRegistry({putActive:[{id:'example.count',version:'1.0.0',digest:'a'.repeat(64)}]},first.revision);
        let conflict=false;try{await s.commitRegistry({deleteActive:['example.count']},first.revision);}catch{conflict=true;}
        const a={workspaceId:'default',pluginId:'example.count'},b={workspaceId:'default',pluginId:'example.other'};
        await s.kvPut(a,'x','original');await s.kvPut(b,'x','other');let quota=false;try{await s.kvPut(a,'x','z'.repeat(1024*1024));}catch{quota=true;}
        const out={conflict,quota,a:await s.kvGet(a,'x'),b:await s.kvGet(b,'x'),active:(await s.loadRegistry()).active.length};s.close();return out;
    });
    expect(result).toEqual({conflict:true,quota:true,a:'original',b:'other',active:1});
});

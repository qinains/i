import {test,expect} from './fixtures.mjs';
import {pluginPackage} from './plugin-fixtures.mjs';
test('plugin lifecycle validates installation grants locks update failures and immutable versions',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const data={one:await pluginPackage(),two:await pluginPackage({version:'1.0.1'})};
    const result=await page.evaluate(async data=>{
        const {openPluginStore}=await import('/plugins/store.js'),{PluginManager}=await import('/plugins/manager.js'),{inspectPluginPackage}=await import('/plugins/runner.js');
        const store=await openPluginStore();let fail=false;const original=store.commitRegistry;store.commitRegistry=(...args)=>{if(fail)throw new Error('Injected commit failure');return original(...args);};
        const manager=new PluginManager({store,inspectPackage:inspectPluginPackage});window.testPlugins=manager;window.testPluginStore=store;
        const preview=await manager.preview(new Uint8Array(data.one));preview.manifest.id='untrusted.ui-change';const installed=await manager.install(preview);
        let initial=await manager.withSnapshot('default',s=>s.tools.length);await manager.enable('default',installed.id,{workspace:'read',storage:false});
        let locked=false;const enabled=await manager.withSnapshot('default',async s=>{try{await manager.uninstall(installed.id);}catch{locked=true;}return s.tools.length;});
        const second=await manager.preview(new Uint8Array(data.two));fail=true;let aborted=false;try{await manager.install(second);}catch{aborted=true;}fail=false;
        const preserved=(await manager.list())[0].version;await manager.install(second);const updated=(await manager.list())[0].version;
        return {id:installed.id,initial,enabled,locked,aborted,preserved,updated,afterUpdate:await manager.withSnapshot('default',s=>s.tools.length)};
    },data);
    expect(result).toEqual({id:'example.word-count',initial:0,enabled:1,locked:true,aborted:true,preserved:'1.0.0',updated:'1.0.1',afterUpdate:0});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    expect(await page.evaluate(async()=>{const {openPluginStore}=await import('/plugins/store.js');const s=await openPluginStore(),r=await s.loadRegistry();s.close();return r.active.map(p=>p.version);})).toEqual(['1.0.1']);
});

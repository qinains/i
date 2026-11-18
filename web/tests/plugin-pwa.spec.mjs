import {test,expect} from './fixtures.mjs';
import {pluginPackage} from './plugin-fixtures.mjs';
test('plugin PWA persists Rust packages grants and OPFS through offline subpath reload',async({page,context})=>{
    await page.goto('/sub/');await expect(page.locator('#status')).toHaveText('Ready');await page.evaluate(async()=>{await navigator.serviceWorker.ready;if(!navigator.serviceWorker.controller)await new Promise(r=>navigator.serviceWorker.addEventListener('controllerchange',r,{once:true}));});
    await page.evaluate(async bytes=>{
        const {openPluginStore}=await import('./plugins/store.js'),{PluginManager}=await import('./plugins/manager.js'),{inspectPluginPackage}=await import('./plugins/runner.js'),fs=await import('./platform.js');
        await fs.opfsWrite(await fs.opfsOpen(),'offline.txt',new TextEncoder().encode('offline plugin'));
        const s=await openPluginStore(),m=new PluginManager({store:s,inspectPackage:inspectPluginPackage}),p=await m.install(await m.preview(new Uint8Array(bytes)));await m.enable('default',p.id,{workspace:'read',storage:false});s.close();
    },await pluginPackage());
    await context.setOffline(true);await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    const result=await page.evaluate(async()=>{
        const {openPluginStore}=await import('./plugins/store.js'),{PluginManager}=await import('./plugins/manager.js'),{inspectPluginPackage,createPluginRun}=await import('./plugins/runner.js'),{createPluginBroker}=await import('./plugins/broker.js');
        const s=await openPluginStore(),m=new PluginManager({store:s,inspectPackage:inspectPluginPackage});
        const result=await m.withSnapshot('default',async snapshot=>{const descriptor=snapshot.tools[0],bytes=await m.loadModule(descriptor);const broker=createPluginBroker({snapshot,descriptor,store:s,context:{workspaceId:'default',sessionId:'offline',mode:'auto',signal:new AbortController().signal,pauseBudget:()=>{}}});return createPluginRun({descriptor,bytes,request:{id:'offline',event:'tool',workspace_id:'default',data:{name:'count_words',arguments:{path:'offline.txt'}}},onHostCall:broker}).result;});s.close();return JSON.parse(result.output);
    });
    expect(result).toEqual({characters:14,words:2});
    const urls=await page.evaluate(async()=>(await Promise.all((await caches.keys()).map(async name=>(await(await caches.open(name)).keys()).map(r=>r.url)))).flat());expect(urls.some(url=>url.endsWith('.hplugin.zip')||url.endsWith('/offline.txt'))).toBe(false);
});

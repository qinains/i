import {test as base,expect} from '@playwright/test';
import http from 'node:http';
const usedWebKitPorts=new Set();

// WebKit's ephemeral context rejects real OPFS on macOS; each test gets its
// own persistent profile rather than mocking away the storage boundary.
export const test = base.extend({
    context: async ({browser,browserName,playwright,baseURL,viewport,acceptDownloads,hasTouch,locale},use,testInfo) => {
        let alias;
        if (browserName === 'webkit') {
            // macOS WebKit also shares OPFS across persistent profiles with the
            // same origin. An ephemeral loopback port isolates each test origin.
            alias = http.createServer((req,res) => {
                const upstream = http.request(new URL(req.url,baseURL),{method:req.method,headers:req.headers},response => {
                    res.writeHead(response.statusCode,response.headers); response.pipe(res);
                });
                upstream.on('error',() => res.destroy()); res.on('close',() => upstream.destroy()); req.pipe(upstream);
            });
            await new Promise(resolve => alias.listen(0,'127.0.0.1',resolve));
            // macOS can recycle a just-closed ephemeral port within one run.
            // OPFS is origin-scoped even across these persistent profiles.
            for(let attempts=0;usedWebKitPorts.has(alias.address().port);attempts++){
                if(attempts>=64)throw new Error('Cannot allocate a fresh WebKit test origin.');
                await new Promise(resolve=>alias.close(resolve));
                await new Promise(resolve=>alias.listen(0,'127.0.0.1',resolve));
            }
            usedWebKitPorts.add(alias.address().port);
        }
        const options = {baseURL:alias ? `http://127.0.0.1:${alias.address().port}` : baseURL,viewport,acceptDownloads,hasTouch,locale,
            // Playwright routing cannot intercept requests owned by a worker.
            // PWA suites exercise workers; model-routing suites isolate that layer.
            serviceWorkers:testInfo.file.endsWith('pwa.spec.mjs')?'allow':'block'};
        const context = browserName === 'webkit'
            ? await playwright.webkit.launchPersistentContext(testInfo.outputPath('webkit-profile'),{...options,headless:true})
            : await browser.newContext(options);
        try {
            // Other suites start after the welcome flow; onboarding has its own coverage.
            if(!testInfo.file.endsWith('onboarding.spec.mjs'))await context.addInitScript(()=>{if(window.top===window)localStorage.setItem('i_harness_welcome_seen','1');});
            if(alias){
                // Ports can also recur across worker restarts or separate runs.
                // Reset only this reserved test origin, once before app startup;
                // reloads within the test must retain the actual OPFS contents.
                const setup=await context.newPage();
                try{
                    await setup.route('**/__harness_test_storage__',route=>route.fulfill({contentType:'text/html',body:'<!doctype html><title>Test storage setup</title>'}));
                    await setup.goto('/__harness_test_storage__');
                    await setup.evaluate(async()=>{const root=await navigator.storage.getDirectory();for await(const name of root.keys())await root.removeEntry(name,{recursive:true});});
                }finally{await setup.close();}
            }
            await use(context);
        } finally {await context.close(); if (alias) {alias.closeAllConnections(); await new Promise(resolve => alias.close(resolve));}}
    },
});
export {expect};

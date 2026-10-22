import {test as base,expect} from '@playwright/test';
import http from 'node:http';

// WebKit's ephemeral context rejects real OPFS on macOS; each test gets its
// own persistent profile rather than mocking away the storage boundary.
export const test = base.extend({
    context: async ({browser,browserName,playwright,baseURL,viewport,acceptDownloads},use,testInfo) => {
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
        }
        const options = {baseURL:alias ? `http://127.0.0.1:${alias.address().port}` : baseURL,viewport,acceptDownloads};
        const context = browserName === 'webkit'
            ? await playwright.webkit.launchPersistentContext(testInfo.outputPath('webkit-profile'),{...options,headless:true})
            : await browser.newContext(options);
        try {await use(context);} finally {await context.close(); if (alias) {alias.closeAllConnections(); await new Promise(resolve => alias.close(resolve));}}
    },
});
export {expect};

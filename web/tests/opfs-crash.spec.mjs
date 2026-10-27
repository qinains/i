import {test,expect} from '@playwright/test';
import {once} from 'node:events';

// A separate persistent browser owns only this test's profile. Never kill the
// shared Playwright browser or a user's browser; CDP identifies our own process.
test('browser process death before close preserves committed OPFS bytes and releases locks',async({playwright,browserName,baseURL},testInfo)=>{
    test.skip(browserName!=='chromium','Process-level crash injection uses Chromium CDP.');
    test.setTimeout(90000);
    const profile=testInfo.outputPath('crash-profile');
    let context;
    const launch=()=>playwright.chromium.launchPersistentContext(profile,{headless:true,serviceWorkers:'block'});
    try{
        context=await launch();let page=await context.newPage();
        await page.goto(`${baseURL}/__harness_test_storage__`);
        await page.evaluate(async()=>{
            const fs=await import('/platform.js'),root=await fs.opfsOpen();
            await fs.opfsWrite(root,'durable.txt',new TextEncoder().encode('committed'));
            await fs.opfsWrite(root,'sibling.txt',new Uint8Array([0,255,10]));
            // Pause the real host write after the real writable accepts data,
            // but before close can publish it. No simulated filesystem is used.
            const original=FileSystemFileHandle.prototype.createWritable;
            FileSystemFileHandle.prototype.createWritable=async function(...args){
                const writer=await original.apply(this,args);
                return {write:writer.write.bind(writer),abort:writer.abort.bind(writer),close:async()=>{window.beforeCommit=true;await new Promise(()=>{});}};
            };
            window.writeOutcome='pending';
            window.pendingWrite=fs.opfsWrite(root,'durable.txt',new TextEncoder().encode('uncommitted'))
                .then(()=>window.writeOutcome='saved',()=>window.writeOutcome='failed');
        });
        await page.waitForFunction(()=>window.beforeCommit===true);
        expect(await page.evaluate(()=>window.writeOutcome)).toBe('pending');
        const session=await context.browser().newBrowserCDPSession();
        const {processInfo}=await session.send('SystemInfo.getProcessInfo');
        const pid=processInfo.find(info=>info.type==='browser')?.id;
        expect(Number.isSafeInteger(pid)&&pid>1&&pid!==process.pid).toBe(true);
        const closed=once(context,'close');
        process.kill(pid,'SIGKILL');await closed;
        context=await launch();page=await context.newPage();
        await page.goto(`${baseURL}/__harness_test_storage__`);
        expect(await page.evaluate(async()=>{
            const fs=await import('/platform.js'),root=await fs.opfsOpen();
            return {text:new TextDecoder().decode(await fs.opfsRead(root,'durable.txt',100)),binary:[...await fs.opfsRead(root,'sibling.txt',10)]};
        })).toEqual({text:'committed',binary:[0,255,10]});
        await page.evaluate(async()=>{
            const fs=await import('/platform.js');await fs.opfsWrite(await fs.opfsOpen(),'durable.txt',new TextEncoder().encode('next commit'));
        });
        await page.reload();
        expect(await page.evaluate(async()=>{
            const fs=await import('/platform.js');return new TextDecoder().decode(await fs.opfsRead(await fs.opfsOpen(),'durable.txt',100));
        })).toBe('next commit');
    }finally{await context?.close();}
});

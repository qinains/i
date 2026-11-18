// Opt-in benchmark; generates synthetic state in disposable Chromium contexts.
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {access} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright';

const runs=Number(process.env.BENCH_RUNS??5);
if(!Number.isInteger(runs)||runs<1||runs>30)throw new Error('BENCH_RUNS must be 1..30');
const cases=[
    {name:'one-small',count:1,characters:128*1024},
    {name:'many-small',count:100,characters:128*1024},
    {name:'one-long-history',count:1,characters:4*1024*1024,historyOnly:true},
    {name:'one-long-plain-line',count:1,characters:4*1024*1024},
];
const summary=items=>{
    const sorted=[...items].sort((a,b)=>a-b),quantile=p=>Math.round(sorted[Math.ceil(sorted.length*p)-1]*10)/10;
    return {p50:quantile(.5),p95:quantile(.95),max:quantile(1),samples:sorted.length};
};
await access(new URL('./pkg/i_bg.wasm',import.meta.url));
const server=spawn(process.execPath,[fileURLToPath(new URL('./serve.mjs',import.meta.url))],{
    env:{...process.env,PORT:'0'},stdio:['ignore','pipe','inherit'],
});
const stop=once(server,'exit');
let browser;
try{
    const origin=await Promise.race([
        once(server.stdout,'data').then(([data])=>String(data).match(/http:\/\/127\.0\.0\.1:\d+/u)?.[0]),
        stop.then(()=>{throw new Error('Static server exited before listening');}),
    ]);
    if(!origin)throw new Error('Cannot determine static server address');
    browser=await chromium.launch();
    const results=[];
    for(const benchmark of cases){
        const coldMs=[],heapMiB=[],writeMs=[];let recordsBytes=0;
        for(let run=0;run<runs;run++){
            const context=await browser.newContext({baseURL:origin,serviceWorkers:'block',locale:'en-US'});
            try{
                await context.route('**/__session_bench_seed',route=>route.fulfill({contentType:'text/html',body:'<!doctype html><title>Benchmark seed</title>'}));
                const seed=await context.newPage();await seed.goto('/__session_bench_seed');
                recordsBytes=await seed.evaluate(async({count,characters,historyOnly})=>{
                    const {openStateStore}=await import('/state/store.js'),store=await openStateStore();
                    try{
                        await store.createWorkspace({id:'default',name:'Default',createdAt:1,storageKind:'persistent'});
                        let total=0;
                        for(let index=0;index<count;index++){
                            const text=String(index).padEnd(characters,'x'),id=`bench-${index}`;
                            const record={id,workspaceId:'default',name:id,titleSource:'manual',connectionId:'',connectionIdentity:'',
                                status:'idle',mode:'ask',draft:'',revision:0,journal:null,
                                history:[{role:'system',content:'synthetic benchmark'},{role:'user',content:text}],
                                transcript:[{type:'user',text:historyOnly?'Short visible text':text}]};
                            await store.commitSession(record,0);total+=JSON.stringify(record).length;
                        }
                        await store.commitUi({activeSessionId:'bench-0',activeWorkspaceId:'default',openedSessionIds:['bench-0']});
                        return total;
                    }finally{store.close();}
                },benchmark);
                await seed.close();
                const page=await context.newPage(),cdp=await context.newCDPSession(page);
                await cdp.send('Performance.enable');
                let measuring=true,peak=0;
                const sample=(async()=>{
                    while(measuring){
                        const {metrics}=await cdp.send('Performance.getMetrics');
                        peak=Math.max(peak,metrics.find(metric=>metric.name==='JSHeapUsedSize')?.value??0);
                        await new Promise(resolve=>setTimeout(resolve,20));
                    }
                })();
                const start=performance.now();
                try{
                    await page.goto('/');
                    await page.waitForFunction(()=>document.getElementById('status')?.textContent==='Ready',null,{timeout:60_000});
                    coldMs.push(performance.now()-start);
                }
                finally{measuring=false;await sample;}
                if(!peak)throw new Error('No Chromium JS heap samples');
                heapMiB.push(peak/(1024*1024));
                writeMs.push(...await page.evaluate(async()=>{
                    const {openStateStore}=await import('/state/store.js'),store=await openStateStore();
                    try{
                        const record=(await store.load()).sessions.find(value=>value.id==='bench-0'),times=[];
                        for(let index=0;index<7;index++){
                            record.draft=`synthetic edit ${index}`;
                            const start=performance.now();
                            record.revision=await store.commitSession(structuredClone(record),record.revision);
                            times.push(performance.now()-start);
                        }
                        return times;
                    }finally{store.close();}
                }));
            }finally{await context.close();}
        }
        results.push({name:benchmark.name,records:benchmark.count,approxSnapshotBytes:recordsBytes,
            coldReadyMs:summary(coldMs),sampledStartupJsHeapMiB:summary(heapMiB),saveMs:summary(writeMs)});
    }
    process.stdout.write(JSON.stringify({browser:browser.version(),platform:process.platform,architecture:process.arch,runs,results},null,2)+'\n');
}finally{
    try{await browser?.close();}
    finally{if(server.exitCode===null&&server.signalCode===null)server.kill();await stop;}
}

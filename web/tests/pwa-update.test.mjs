import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';

const workerSource=await readFile(new URL('../sw.js',import.meta.url),'utf8');
const pageSource=(await readFile(new URL('../pwa.js',import.meta.url),'utf8'))
    .replace('export async function registerPwa','async function registerPwa')
    .replace('import.meta.url',JSON.stringify('https://example.test/app/pwa.js'));
const settle=()=>new Promise(resolve=>setImmediate(resolve));
function clock(){
    let now=0,next=0;const timers=new Map();
    return {
        setTimeout(fn,delay){const id=++next;timers.set(id,{fn,at:now+delay});return id;},
        clearTimeout(id){timers.delete(id);},
        async advance(ms){now+=ms;for(const [id,item]of [...timers])if(item.at<=now){timers.delete(id);item.fn();}await settle();},
        get count(){return timers.size;},
    };
}
function worker(){
    const time=clock(),events={},messages=[],scope='https://example.test/app/';let activation=0,serial=0;
    const pages=['one','two'].map(id=>({id,url:scope,postMessage(data){messages.push({page:id,...data});}}));
    const self={HARNESS_BUILD:{version:'fixture',precache:[]},registration:{scope},clients:{async matchAll(){return [...pages];}},addEventListener(type,fn){events[type]=fn;},async skipWaiting(){activation++;}};
    vm.runInNewContext(workerSource,{self,URL,crypto:{randomUUID:()=>String(++serial)},setTimeout:time.setTimeout,clearTimeout:time.clearTimeout});
    const message=(page,data)=>{let pending;events.message({source:page,data,waitUntil(value){pending=value;}});return pending;};
    return {time,pages,messages,get activation(){return activation;},start:()=>message(pages[0],{type:'pwa_update'}),ack:(page,idle,reason,id='1',protocol=2)=>message(page,{type:'pwa_idle',id,idle,reason,protocol})};
}

test('update allows a slow save beyond three seconds, then activates only after every acknowledgement',async()=>{
    const w=worker(),pending=w.start();await settle();w.ack(w.pages[0],true);
    await w.time.advance(5000);
    assert.equal(w.activation,0);assert.equal(w.messages.some(item=>item.type==='pwa_unlock'),false);
    w.ack(w.pages[1],true);await pending;assert.equal(w.activation,1);assert.equal(w.time.count,0);
});
test('busy page postpones promptly with a specific reason and releases all response timers',async()=>{
    const w=worker(),pending=w.start();await settle();w.ack(w.pages[1],false,'busy');await settle();
    assert.equal(w.messages.find(item=>item.type==='pwa_unlock')?.reason,'busy');
    await pending;assert.equal(w.activation,0);assert.equal(w.time.count,0);
});
test('nonresponding open page prevents activation after the longer deadline and can be retried',async()=>{
    const w=worker(),pending=w.start();await settle();w.ack(w.pages[0],true);
    await w.time.advance(30000);await pending;
    assert.equal(w.messages.find(item=>item.type==='pwa_unlock')?.reason,'unresponsive');assert.equal(w.activation,0);
    const retry=w.start();await settle();w.ack(w.pages[0],true,undefined,'1');await settle();assert.equal(w.activation,0);
    w.ack(w.pages[0],true,undefined,'2');w.ack(w.pages[1],true,undefined,'2');await retry;assert.equal(w.activation,1);
});
test('a page closed while waiting does not block the update',async()=>{
    const w=worker(),pending=w.start();await settle();w.ack(w.pages[0],true);w.pages.pop();
    await w.time.advance(30000);await pending;assert.equal(w.activation,1);
});
test('old pages with ten-second locks cannot grant readiness for a thirty-second check',async()=>{
    const w=worker(),pending=w.start();await settle();w.ack(w.pages[0],true,undefined,'1',1);
    await w.time.advance(9000);assert.equal(w.messages.find(item=>item.type==='pwa_unlock')?.reason,'unresponsive');
    await pending;w.ack(w.pages[1],true);assert.equal(w.activation,0);assert.equal(w.time.count,0);
});
test('a discarded page cannot prevent other pages from being unlocked',async()=>{
    const w=worker();w.pages[1].postMessage=()=>{throw new Error('Discarded client');};
    const pending=w.start();await settle();w.ack(w.pages[0],true);await pending;
    assert.equal(w.activation,0);assert.equal(w.messages.find(item=>item.type==='pwa_unlock')?.reason,'unresponsive');assert.equal(w.time.count,0);
});
test('save failure and newly opened pages postpone without activating',async()=>{
    for(const reason of ['save-failed','changed']){
        const w=worker(),pending=w.start();await settle();
        if(reason==='changed')w.pages.push({id:'new',url:w.pages[0].url,postMessage(data){w.messages.push({page:'new',...data});}});
        w.ack(w.pages[0],true);w.ack(w.pages[1],reason!=='save-failed',reason);
        await pending;assert.equal(w.activation,0);assert.equal(w.messages.find(item=>item.type==='pwa_unlock')?.reason,reason);
    }
});

async function page(beforeUpdate=async()=>{}){
    const time=clock(),events={},buttons={},statuses=[],locks=[],posts=[];let busy=false,reloads=0;
    for(const id of ['btn-install','btn-update'])buttons[id]={hidden:true,disabled:false,addEventListener(type,fn){this[type]=fn;}};
    const source={scriptURL:'https://example.test/app/service-worker.js',postMessage(data){posts.push(data);}};
    const registration={active:{state:'activated'},waiting:source,addEventListener(){}};
    const context=vm.createContext({URL,isSecureContext:true,document:{getElementById:id=>buttons[id]},navigator:{onLine:true,serviceWorker:{addEventListener(type,fn){events[type]=fn;},async register(){return registration;}}},window:{addEventListener(){}},location:{reload(){reloads++;}},setTimeout:time.setTimeout,clearTimeout:time.clearTimeout});
    vm.runInContext(pageSource,context);
    await context.registerPwa({isBusy:()=>busy,onLock:value=>locks.push(value),onStatus:text=>statuses.push(text),beforeUpdate});
    return {time,buttons,statuses,locks,posts,source,registration,get reloads(){return reloads;},set busy(value){busy=value;},message:data=>events.message({source,data}),change:()=>events.controllerchange()};
}
test('an update button is hidden when no waiting worker remains after first installation',async()=>{
    const p=await page();assert.equal(p.buttons['btn-update'].hidden,false);
    p.registration.waiting=null;p.change();assert.equal(p.buttons['btn-update'].hidden,true);
});
test('late save completion after cancellation cannot re-lock or acknowledge an expired attempt',async()=>{
    let finish;const p=await page(()=>new Promise(resolve=>{finish=resolve;}));
    const pending=p.message({type:'pwa_lock',id:'old'});await settle();assert.equal(p.locks.at(-1),true);
    await p.message({type:'pwa_unlock',id:'old',reason:'unresponsive'});finish();await pending;
    assert.equal(p.locks.at(-1),false);assert.equal(p.posts.length,0);assert.equal(p.time.count,0);
    p.change();assert.equal(p.reloads,0);
});
test('apply update is single-flight, shows reason-specific guidance, and can be retried',async()=>{
    const p=await page();await p.buttons['btn-update'].click();await p.buttons['btn-update'].click();
    assert.equal(p.posts.length,1);assert.equal(p.buttons['btn-update'].disabled,true);
    await p.message({type:'pwa_lock',id:'attempt'});await p.message({type:'pwa_unlock',id:'attempt',reason:'busy'});
    assert.match(p.statuses.at(-1),/another app tab is running/u);assert.equal(p.buttons['btn-update'].disabled,false);
    await p.buttons['btn-update'].click();assert.equal(p.posts.filter(item=>item.type==='pwa_update').length,2);
});
test('save failure reports its own reason; successful save keeps the page locked until activation',async()=>{
    const failed=await page(async()=>{throw new Error('fixture');});await failed.message({type:'pwa_lock',id:'failed'});
    assert.equal(failed.posts.at(-1).reason,'save-failed');assert.equal(failed.locks.at(-1),false);
    const good=await page();await good.message({type:'pwa_lock',id:'saved'});assert.equal(good.posts.at(-1).idle,true);
    good.change();assert.equal(good.reloads,1);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import {modelDeadline} from '../model-deadline.js';
function clock(){let now=0,id=0;const tasks=new Map();return {setTimeout(fn,ms){tasks.set(++id,{fn,at:now+ms});return id;},clearTimeout(id){tasks.delete(id);},advance(ms){now+=ms;for(const [id,task]of tasks)if(task.at<=now){tasks.delete(id);task.fn();}},get size(){return tasks.size;}};}
test('model deadline is inactivity-based, not a total duration cap',()=>{
    const timers=clock(),d=modelDeadline(new AbortController().signal,1800000,timers);
    for(let i=0;i<10;i++){timers.advance(1200000);assert.equal(d.signal.aborted,false);d.touch();}
    timers.advance(1800000);assert.equal(d.signal.reason.name,'TimeoutError');d.dispose();assert.equal(timers.size,0);
});
test('manual cancellation and disposal clear all deadline timers',()=>{
    const timers=clock(),parent=new AbortController(),d=modelDeadline(parent.signal,10,timers);
    parent.abort();assert.equal(d.signal.reason.name,'AbortError');assert.equal(timers.size,0);d.touch();assert.equal(timers.size,0);d.dispose();
    const completed=modelDeadline(new AbortController().signal,10,timers);completed.dispose();timers.advance(10);assert.equal(completed.signal.aborted,false);
});

import assert from 'node:assert/strict';
import {test} from 'node:test';
import {createBrowserApplication} from '../application.js';

const record={id:'session',workspaceId:'default',history:[{role:'system',content:'keep'}]};
test('one application owns sessions and plugin storage and closes state after checkpoints',async()=>{
    const entered=Promise.withResolvers(),release=Promise.withResolvers(),events=[];
    const app=createBrowserApplication({store:{close(){events.push('state');}},hooks:{},
        Agent:{create_workspace:async()=>({import_history(){},cancel(){},async close(){entered.resolve();await release.promise;events.push('checkpoint');},free(){events.push('free');}})},
        openPluginStore:async()=>({close(){events.push('plugins');}}),inspectPackage:async()=>({})});
    await app.controller.agent(record);const plugins=await app.plugins();
    assert.equal(plugins.context.fiber.parent.fiber,app.context.fiber);
    const closing=app.shutdown();assert.equal(app.shutdown(),closing);await entered.promise;
    assert.ok(!events.includes('state'));
    await assert.rejects(app.plugins(),/closing/u);
    release.resolve();assert.equal((await closing).closed,true);
    assert.ok(events.indexOf('state')>events.indexOf('free'));
    assert.equal(events.filter(event=>event==='state').length,1);
    await assert.rejects(app.controller.agent(record),/closing/u);
});

test('application timeout retains storage until independently owned cleanup completes',async()=>{
    const release=Promise.withResolvers(),closed=Promise.withResolvers(),events=[];
    const app=createBrowserApplication({store:{close(){events.push('state');closed.resolve();}},hooks:{},
        Agent:{create_workspace:async()=>({import_history(){},cancel(){},close:()=>release.promise,free(){events.push('free');}})}});
    await app.controller.agent(record);
    const report=await app.shutdown({timeoutMs:10});assert.equal(report.closed,false);assert.deepEqual(events,[]);
    release.resolve();await closed.promise;assert.deepEqual(events,['free','state']);
});

test('ancestor disposal owns application closure, including a late plugin store',async()=>{
    const acquired=Promise.withResolvers(),entered=Promise.withResolvers(),events=[];
    const app=createBrowserApplication({Agent:{},hooks:{},store:{close(){events.push('state');}},
        openPluginStore:()=>{entered.resolve();return acquired.promise;},inspectPackage:async()=>({})});
    const plugins=app.plugins();await entered.promise;const closing=app.context.fiber.dispose();
    acquired.resolve({close(){events.push('plugins');}});
    await assert.rejects(plugins,/closing/u);await closing;
    assert.deepEqual(events,['plugins','state']);
    await assert.rejects(app.controller.create('default','late'),/closing/u);
});

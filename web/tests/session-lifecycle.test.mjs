import assert from 'node:assert/strict';
import {test} from 'node:test';
import {SessionController} from '../sessions.js';
import {Context} from '../plugins/vendor/cordis.js';
const record=id=>({id,workspaceId:'default',history:[{role:'system',content:'keep'}]});

test('shutdown during lazy initialization closes the late adapter instead of publishing it', async () => {
  const ready = Promise.withResolvers(); const events = [];
  const entered=Promise.withResolvers(),context=new Context();
  const agent = {cancel(){},async close() { events.push('close'); }, free() { events.push('free'); }, import_history() { events.push('import'); }};
  const controller = new SessionController({create_workspace: () => {entered.resolve();return ready.promise;}}, {}, {},context);
  const record = {id: 'late', workspaceId: 'default', history: [{role: 'system', content: 'keep'}]};
  const starting = controller.agent(record);
  await entered.promise;
  const closing = controller.shutdown();
  await assert.rejects(controller.agent(record), /closing/u);
  ready.resolve(agent);
  await assert.rejects(starting, /closing/u); await closing;
  assert.deepEqual(events, ['close', 'free']); assert.equal(controller.agents.size, 0);
  await context.fiber.dispose();
});

test('session shutdown drains checkpoints and joins one close before freeing', async () => {
  const drain = Promise.withResolvers(), entered=Promise.withResolvers(),events = [],context=new Context();
  const controller = new SessionController({create_workspace:async()=>({import_history(){},cancel(){},async close() { events.push('close');entered.resolve(); await drain.promise; }, free() { events.push('free'); }})}, {}, {},context);
  await controller.agent(record('a'));
  const closing = controller.shutdown(); assert.equal(controller.shutdown(), closing);
  await entered.promise; assert.deepEqual(events, ['close']);
  drain.resolve(); const report = await closing;
  assert.deepEqual(report.errors, []); assert.deepEqual(events, ['close', 'free']);
  await context.fiber.dispose();
});

test('shutdown timeout reports retained adapter and does not free resources still in use', async () => {
  const release = Promise.withResolvers(); let freed = 0;
  const context=new Context(),controller = new SessionController({create_workspace:async()=>({import_history(){},cancel(){},close: () => release.promise, free() { freed++; }})}, {}, {},context);
  await controller.agent(record('blocked'));
  const report = await controller.shutdown({timeoutMs: 10});
  assert.equal(report.closed, false); assert.deepEqual(report.retained, ['blocked']); assert.equal(freed, 0);
  release.resolve(); await controller.close('blocked'); assert.equal(freed, 1);
  await context.fiber.dispose();
});

test('closing a never-activated session still flushes its deferred durable draft',async()=>{
  const context=new Context(),saved=[];
  const controller=new SessionController({create_workspace:async()=>{throw new Error('must not acquire');}},
    {async commitSession(record,revision){saved.push(record.draft);return revision+1;}},{},context);
  const session={...record('pending'),revision:0,draft:'keep this draft'};
  controller.records.set(session.id,session);controller.later(session);
  const starting=controller.agent(session);const closing=controller.close(session.id);
  await assert.rejects(starting,/closing/u);await closing;
  assert.deepEqual(saved,['keep this draft']);assert.equal(controller.timers.size,0);
  await context.fiber.dispose();
});

import test from 'node:test';
import assert from 'node:assert/strict';
import {createApplicationPlugins} from '../plugins/application.js';
import {Context, Service} from '../plugins/vendor/cordis.js';

test('application mounts the real manager and owns the real store lifecycle', async () => {
  let closed = 0;
  const store = {close() { closed++; }};
  const app = await createApplicationPlugins({store, inspectPackage: async bytes => ({
    manifest: {id: 'example', version: '1.0.0'}, digest: 'abc', license: 'MIT',
  })});
  const manager = app.context.get('pluginManager');
  assert.ok(app.context instanceof Context);
  assert.ok(manager instanceof Service);
  const preview = await manager.preview(new Uint8Array([1, 2]));
  assert.equal(preview.manifest.id, 'example');
  await app.shutdown();
  assert.equal(app.context.get('pluginManager'), undefined);
  assert.equal(closed, 1);
  await app.shutdown();
  assert.equal(closed, 1);
});

test('shutdown drains a real manager operation before closing its store and rejects new calls', async () => {
  const entered = Promise.withResolvers(), release = Promise.withResolvers();
  const events = [];
  const app = await createApplicationPlugins({store: {close() { events.push('close'); }}, inspectPackage: async () => {
    entered.resolve(); await release.promise; events.push('inspect');
    return {manifest: {id: 'example', version: '1.0.0'}, digest: 'abc', license: 'MIT'};
  }});
  const manager = app.manager, preview = manager.preview(new Uint8Array([1]));
  await entered.promise;
  const closing = app.shutdown();
  assert.equal(app.shutdown(), closing);
  await assert.rejects(manager.preview(new Uint8Array([2])), /closing/u);
  assert.deepEqual(events, []);
  release.resolve(); await preview; await closing;
  assert.deepEqual(events, ['inspect', 'close']);
  assert.throws(() => app.manager, /closed/u);
});

test('manager activation waits for asynchronous store acquisition', async () => {
  const ready = Promise.withResolvers(); let closed = 0, initialized = false;
  const pending = createApplicationPlugins({store: ready.promise, inspectPackage: async () => ({manifest: {id: 'example'}})});
  pending.then(() => { initialized = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(initialized, false);
  ready.resolve({close() { closed++; }});
  const app = await pending; assert.ok(app.manager instanceof Service);
  await app.shutdown(); assert.equal(closed, 1);
});

test('store cleanup failures are reported without reopening terminal application', async () => {
  const app = await createApplicationPlugins({store: {close() { throw new Error('storage close failed'); }}, inspectPackage: async () => ({})});
  const report = await app.shutdown();
  assert.deepEqual(report.errors, ['storage close failed']);
  assert.throws(() => app.manager, /closed/u);
});

test('plugin boundary shares its application root without shutting down siblings',async()=>{
  const root=new Context(),events=[];
  root.use(ctx=>{ctx.effect(()=>()=>{events.push('sibling');});});
  const app=await createApplicationPlugins({context:root,store:{close(){events.push('store');}},inspectPackage:async()=>({})});
  assert.equal(app.context.fiber.parent.fiber,root.fiber);
  await app.shutdown();assert.deepEqual(events,['store']);
  await root.fiber.dispose();assert.deepEqual(events,['store','sibling']);
});

test('ancestor disposal collects a plugin store acquired after closing started',async()=>{
  const root=new Context(),ready=Promise.withResolvers(),entered=Promise.withResolvers();let closed=0;
  const store={then(resolve,reject){entered.resolve();return ready.promise.then(resolve,reject);}};
  const starting=createApplicationPlugins({context:root,store,inspectPackage:async()=>({})});
  await entered.promise;const closing=root.fiber.dispose();
  ready.resolve({close(){closed++;}});
  await assert.rejects(starting,/closing|inactive/u);await closing;assert.equal(closed,1);
});

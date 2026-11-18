import assert from 'node:assert/strict';
import {Context, Service} from '../../plugins/vendor/cordis.js';
import {createEventScenarios} from './cordis-events.mjs';

export function validateScenario(scenario) {
  assert.equal(scenario.version, 1, 'unsupported scenario version');
  assert.equal(typeof scenario.id, 'string');
  assert.ok(Array.isArray(scenario.gates));
  assert.equal(new Set(scenario.gates).size, scenario.gates.length, 'duplicate gate');
  assert.ok(scenario.definitions && typeof scenario.definitions === 'object');
  const shapes = {
    publish: ['op', 'binding', 'service', 'value'],
    mount: ['op', 'definition', 'fiber', 'config'],
    'replace-binding': ['op', 'binding', 'replacement', 'value'],
    settle: ['op', 'fiber'],
    state: ['op', 'fiber'],
    withdraw: ['op', 'binding'],
    instances: ['op', 'definition'],
    dispose: ['op', 'fiber'],
    'wait-gate': ['op', 'gate'],
    'open-gate': ['op', 'gate'],
    update: ['op', 'fiber', 'config'],
    identity: ['op', 'fiber'],
    'replace-component': ['op', 'definition', 'replacement'],
    isolate: ['op', 'parent', 'target', 'service', 'realm'],
    'publish-in': ['op', 'context', 'binding', 'service', 'value'],
    'mount-in': ['op', 'context', 'definition', 'fiber', 'config'],
    counter: ['op', 'service'],
    count: ['op', 'service'],
    listen: ['op', 'event', 'listener', 'behavior'],
    dispatch: ['op', 'event', 'mode'],
    'drain-bindings': ['op'],
    tagged: ['op', 'service'],
    intercept: ['op', 'parent', 'target', 'service', 'tags'],
  };
  for (const action of scenario.actions) {
    assert.ok(Object.hasOwn(shapes, action.op), `unknown action: ${action.op}`);
    assert.deepEqual(Object.keys(action).sort(), shapes[action.op].slice().sort(), 'invalid action fields');
    for (const name of shapes[action.op].filter(key => !['value', 'config', 'tags'].includes(key))) {
      assert.equal(typeof action[name], 'string', `invalid action ${name}`);
    }
    if (Object.hasOwn(action, 'gate')) assert.ok(scenario.gates.includes(action.gate), 'unknown gate');
  }
  for (const spec of Object.values(scenario.definitions)) {
    for (const key of ['load_gate', 'unload_gate', 'ready_gate']) {
      if (Object.hasOwn(spec, key)) assert.ok(scenario.gates.includes(spec[key]), 'unknown gate');
    }
  }
  const edges = new Map();
  for (const [a, b] of scenario.before) {
    edges.set(a, [...edges.get(a) ?? [], b]);
  }
  const visiting = new Set(), visited = new Set();
  function visit(node) {
    assert.ok(!visiting.has(node), 'ordering cycle');
    if (visited.has(node)) return;
    visiting.add(node);
    for (const next of edges.get(node) ?? []) visit(next);
    visiting.delete(node); visited.add(node);
  }
  for (const key of edges.keys()) visit(key);
  return scenario;
}

export async function runScenario(scenario) {
  validateScenario(scenario);
  const root = new Context(), events = [], errors = [];
  const bindings = new Map(), fibers = new Map(), retiring = [];
  const identities = new Map();
  const contexts = new Map([['root', root]]), realms = new Map();
  const eventAction = createEventScenarios(root, events, errors);
  class Tagged extends Service {
    constructor(ctx, name) { super(ctx, name); }
    [Service.mergeIntercept](...layers) { return layers.flat(); }
    tags() { return this[Service.resolveIntercept](); }
  }
  class Facade extends Service {
    constructor(ctx, name, dependency) { super(ctx, name); this.dependency = dependency; }
    tags() { return this.ctx[this.dependency].tags(); }
  }
  class Counter extends Service {
    constructor(ctx, name) { super(ctx, name); this.value = 0; }
    async increase() {
      const self = this;
      await this.ctx.effect(async function* () {
        for (const name of ['first', 'second']) {
          self.value++;
          yield () => { self.value--; events.push(`release(${name})`); };
        }
      });
    }
  }
  const gates = new Map(scenario.gates.map(name => {
    let enter, open;
    const entered = new Promise(resolve => { enter = resolve; });
    const opened = new Promise(resolve => { open = resolve; });
    return [name, {entered, open, async wait() {
      events.push(`gate(${name})`); enter(); await opened;
    }}];
  }));
  const definitions = new Map(Object.entries(scenario.definitions).map(([name, spec]) => [name, {
    name, inject: spec.inject_tags ? Object.fromEntries(spec.inject.map(key => [key, spec.inject_tags[key] ?? true])) : spec.inject,
    async apply(ctx, config) {
      if (spec.kind === 'facade') { new Facade(ctx, spec.provide, spec.read); return; }
      if (spec.kind === 'metadata') {
        for (const [key, tags] of Object.entries(spec.use_tags ?? {})) ctx = ctx.intercept(key, tags);
        events.push(`tags(${ctx[spec.read].tags().join(',')})`); return;
      }
      if (spec.kind === 'counter') {
        events.push(`${spec.load}()`);
        await ctx[spec.read].increase();
        return () => { events.push(`${spec.unload}()`); };
      }
      if (spec.load_gate) await gates.get(spec.load_gate).wait();
      const label = () => spec.config_label ? config[spec.config_label] : ctx[spec.read].version;
      events.push(`${spec.load}(${label()})`);
      if (spec.provide) ctx.provide(spec.provide, {version: ctx[spec.read].version});
      ctx.effect(() => async () => {
        if (spec.unload_gate) await gates.get(spec.unload_gate).wait();
        events.push(`${spec.unload}(${label()})`);
      });
      if (spec.ready_gate) await gates.get(spec.ready_gate).wait();
      if (spec.fail_versions?.includes(ctx[spec.read].version)) throw new Error('activation-failed');
    },
  }]));
  try {
    for (const action of scenario.actions) {
      if (['listen', 'dispatch'].includes(action.op)) { await eventAction(action); continue; }
      const context = action.context ? contexts.get(action.context) : root;
      assert.ok(context, 'unknown context');
      switch (action.op) {
        case 'tagged': new Tagged(context, action.service); break;
        case 'intercept': {
          assert.ok(contexts.has(action.parent), 'unknown parent context');
          assert.ok(!contexts.has(action.target), 'duplicate context');
          contexts.set(action.target, contexts.get(action.parent).intercept(action.service, action.tags)); break;
        }
        case 'drain-bindings': await Promise.all(retiring); break;
        case 'counter': new Counter(context, action.service); break;
        case 'count': events.push(`count(${action.service},${context.get(action.service).value})`); break;
        case 'isolate': {
          assert.ok(contexts.has(action.parent), 'unknown parent context');
          assert.ok(!contexts.has(action.target), 'duplicate context');
          if (!realms.has(action.realm)) realms.set(action.realm, Symbol(action.realm));
          contexts.set(action.target, contexts.get(action.parent).isolate(action.service, realms.get(action.realm)));
          break;
        }
        case 'publish':
        case 'publish-in':
          assert.ok(!bindings.has(action.binding), 'duplicate binding');
          bindings.set(action.binding, {service: action.service, context, withdraw: context.provide(action.service, action.value)});
          break;
        case 'mount':
        case 'mount-in':
          assert.ok(!fibers.has(action.fiber), 'duplicate fiber');
          assert.ok(definitions.has(action.definition), 'unknown definition');
          fibers.set(action.fiber, context.use(definitions.get(action.definition), action.config));
          identities.set(action.fiber, fibers.get(action.fiber).uid);
          break;
        case 'replace-component':
          assert.ok(definitions.has(action.definition) && definitions.has(action.replacement), 'unknown definition');
          root.registry.replace(definitions.get(action.definition), definitions.get(action.replacement)); break;
        case 'identity':
          assert.ok(fibers.has(action.fiber), 'unknown fiber');
          events.push(`identity(${action.fiber},${fibers.get(action.fiber).uid === identities.get(action.fiber)})`); break;
        case 'withdraw': {
          const binding = bindings.get(action.binding); assert.ok(binding, 'unknown binding');
          retiring.push(binding.withdraw()); bindings.delete(action.binding); break;
        }
        case 'dispose':
          assert.ok(fibers.has(action.fiber), 'unknown fiber');
          retiring.push(fibers.get(action.fiber).dispose()); break;
        case 'wait-gate': await gates.get(action.gate).entered; break;
        case 'open-gate': gates.get(action.gate).open(); break;
        case 'update':
          assert.ok(fibers.has(action.fiber), 'unknown fiber');
          retiring.push(Promise.resolve(fibers.get(action.fiber).update(action.config))); break;
        case 'state':
          assert.ok(fibers.has(action.fiber), 'unknown fiber');
          events.push(`state(${action.fiber},${['PENDING','LOADING','ACTIVE','FAILED','DISPOSED','UNLOADING'][fibers.get(action.fiber).state]})`);
          break;
        case 'instances':
          assert.ok(definitions.has(action.definition), 'unknown definition');
          events.push(`instances(${action.definition},${root.registry.get(definitions.get(action.definition))?.fibers.length ?? 0})`);
          break;
        case 'replace-binding': {
          const old = bindings.get(action.binding);
          assert.ok(old, 'unknown binding');
          retiring.push(old.withdraw());
          bindings.delete(action.binding);
          bindings.set(action.replacement, {service: old.service, context: old.context, withdraw: old.context.provide(old.service, action.value)});
          break;
        }
        case 'settle':
          assert.ok(fibers.has(action.fiber), 'unknown fiber');
          try { await fibers.get(action.fiber).await(); }
          catch (error) { errors.push(error.message); }
          await Promise.all(retiring);
          break;
      }
    }
    const names = ['PENDING','LOADING','ACTIVE','FAILED','DISPOSED','UNLOADING'];
    const result = {events: [...events], states: Object.fromEntries([...fibers].map(([id, fiber]) => [id, names[fiber.state]])), errors};
    assert.deepEqual(result.events, scenario.expected, 'trace mismatch');
    assert.deepEqual(result.errors, scenario.expected_errors ?? [], 'error outcome mismatch');
    for (const [a, b] of scenario.before) {
      assert.ok(result.events.includes(a) && result.events.includes(b), 'ordering references absent event');
      assert.ok(result.events.indexOf(a) < result.events.indexOf(b), `ordering mismatch: ${a} before ${b}`);
    }
    return result;
  } finally {
    for (const gate of gates.values()) gate.open();
    await root.fiber.dispose();
  }
}

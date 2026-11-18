import {Context, Service} from './vendor/cordis.js';
import {PluginManager} from './manager.js';

class ManagerService extends Service {
  static inject = ['pluginStore', 'inspectPackage'];

  constructor(ctx) {
    super(ctx, 'pluginManager');
    this._manager = new PluginManager({store: ctx.pluginStore, inspectPackage: ctx.inspectPackage});
    this._state = {closing: false, pending: new Set()};
    ctx.effect(() => () => this.beginClose());
  }

  beginClose() {
    this._state.closing = true;
    return Promise.allSettled([...this._state.pending]);
  }

  async _invoke(method, args) {
    if (this._state.closing) throw new Error('Plugin application is closing.');
    let finished;
    const completion = new Promise(resolve => { finished = resolve; });
    const dispose = this.ctx.effect(() => () => completion);
    const operation = Promise.resolve().then(() => this._manager[method](...args));
    this._state.pending.add(operation);
    try {
      return await operation;
    } finally {
      finished();
      this._state.pending.delete(operation);
      await dispose();
    }
  }
}

for (const method of ['preview', 'install', 'list', 'enable', 'disable', 'uninstall', 'clearData', 'loadModule', 'withSnapshot']) {
  Object.defineProperty(ManagerService.prototype, method, {
    value(...args) { return this._invoke(method, args); },
  });
}

export async function createApplicationPlugins({store, inspectPackage, context: parent = new Context()}) {
  const errors = [];
  let closing, disposed = false;
  const boundary = parent.use(ctx => {
    ctx.effect(() => () => { disposed = true; });
  });
  await boundary;
  const context = boundary.ctx;
  const shutdown = () => {
    if (closing) return closing;
    context.get('pluginManager')?.beginClose();
    closing = (async () => {
      disposed = true;
      await boundary.dispose();
      return {closed: true, errors: [...errors]};
    })();
    return closing;
  };
  try {
    context.provide('inspectPackage', inspectPackage);
    const manager = context.use(ManagerService);
    await context.use(async ctx => {
      let acquired;
      await ctx.effect(async () => {
        acquired = await (typeof store === 'function' ? store() : store);
        return async () => {
          try { await acquired.close(); } catch (error) { errors.push(String(error?.message ?? error)); }
        };
      });
      if (disposed || ctx.fiber.uid === null) throw new Error('Plugin application is closing.');
      ctx.provide('pluginStore', acquired);
    });
    await manager;
    if (disposed || boundary.uid === null) throw new Error('Plugin application is closing.');
  } catch (error) {
    await shutdown();
    if (errors.length) throw new AggregateError([error, ...errors.map(message => new Error(message))], 'Application plugin initialization failed');
    throw error;
  }
  return {
    context,
    get store() { return context.get('pluginStore'); },
    get manager() {
      if (closing || disposed) throw new Error('Plugin application is closed.');
      return context.get('pluginManager');
    },
    shutdown,
  };
}

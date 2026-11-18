import assert from 'node:assert/strict';

export function createEventScenarios(context, events, errors) {
  const subscriptions = new Map();
  return async action => {
    if (action.op === 'listen') {
      assert.ok(!subscriptions.has(action.listener), 'duplicate listener');
      const {listener, behavior} = action;
      const record = () => { events.push(`listener(${listener})`); };
      let callback;
      if (behavior === 'around' || behavior === 'twice') {
        callback = async next => {
          record();
          const value = await next();
          if (behavior === 'twice') await next();
          events.push(`after(${listener})`); return value;
        };
      } else {
        callback = () => {
          record();
          switch (behavior) {
            case 'false': return false;
            case 'null': return null;
            case 'undefined': return;
            case 'zero': return 0;
            case 'empty': return '';
            case 'short': return 'short';
            case 'fail': throw new Error(`failed(${listener})`);
            default:
              assert.ok(behavior.startsWith('remove:'), 'unknown listener behavior');
              assert.ok(subscriptions.has(behavior.slice(7)), 'unknown listener removal');
              subscriptions.get(behavior.slice(7))();
          }
        };
      }
      subscriptions.set(listener, context.on(action.event, callback));
      return true;
    }
    if (action.op !== 'dispatch') return false;
    assert.ok(['emit', 'bail', 'serial', 'parallel', 'waterfall'].includes(action.mode), 'unknown event mode');
    try {
      const args = action.mode === 'waterfall' ? [() => {
        events.push(`tail(${action.event})`); return 'tail';
      }] : [];
      const value = await context[action.mode](action.event, ...args);
      events.push(`result(${action.event},${action.mode},${value === undefined ? 'undefined' : JSON.stringify(value)})`);
    } catch (error) {
      events.push(`error(${action.event},${action.mode})`);
      errors.push(...(error instanceof AggregateError ? error.errors : [error]).map(item => item.message));
    }
    return true;
  };
}

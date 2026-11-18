import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFile} from 'node:fs/promises';

const fixture = JSON.parse(await readFile(new URL('../../tests/fixtures/cordis/binding-replacement.json', import.meta.url)));
const adapter = new URL('./support/cordis-scenarios.mjs', import.meta.url);
test('replacement_teardown_keeps_old_binding', async () => {
  const {runScenario} = await import(adapter);
  const result = await runScenario(fixture);
  assert.deepEqual(result.events, ['load(1)', 'unload(1)', 'load(2)']);
  assert.deepEqual(result.errors, []);
});
for (const name of ['pending-dependency', 'multiple-instances', 'unregistered-consumer', 'withdraw-during-setup', 'independent-loading', 'transitive-drain', 'latest-update-during-setup', 'component-replacement', 'failed-refresh', 'shared-realms', 'consumer-owned-effects', 'event-modes', 'provider-readiness', 'sibling-retirement', 'latest-update-during-unload', 'scoped-interception', 'service-cycle']) test(`shared scenario ${name}`, {timeout: 5000}, async () => {
  const {runScenario} = await import(adapter);
  const scenario = JSON.parse(await readFile(new URL(`../../tests/fixtures/cordis/${name}.json`, import.meta.url)));
  await runScenario(scenario);
});
test('scenario_rejects_unknown_action', async () => {
  const {validateScenario} = await import(adapter);
  assert.throws(() => validateScenario({...fixture, actions: [{op: 'typo'}]}), /unknown action/u);
});
test('scenario_rejects_duplicate_gate', async () => {
  const {validateScenario} = await import(adapter);
  assert.throws(() => validateScenario({...fixture, gates: ['setup', 'setup']}), /duplicate gate/u);
});
test('scenario_rejects_contradictory_ordering', async () => {
  const {validateScenario} = await import(adapter);
  assert.throws(() => validateScenario({...fixture, before: [['a', 'b'], ['b', 'a']]}), /ordering cycle/u);
});
test('oracle_rejects_a_wrong_expected_trace', async () => {
  const {runScenario} = await import(adapter);
  await assert.rejects(runScenario({...fixture, expected: ['load(1)', 'unload(2)', 'load(2)']}), /trace mismatch/u);
});

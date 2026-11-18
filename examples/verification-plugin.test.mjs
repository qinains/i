import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {test} from 'node:test';
import {setImmediate} from 'node:timers/promises';
import {classifyReport, createHandler} from './verification-plugin.mjs';

const check_id = 'browser-session-recovery';
const workspace = resolve('.');
const titles = [
    'reload restores a saved uninitialized conversation and draft without requiring a system message',
    'two windows reject conflicting saves and the stale window can export its unsent draft and attachment',
];
const request = (args, root = workspace) => ({
    event: 'tool', workspace: root, data: {name: 'verify_project_check', arguments: {check_id, ...args}},
});
function report(statuses = ['passed', 'passed']) {
    return {errors: [], suites: [{suites: [{specs: statuses.map((status, i) => ({
        title: titles[i], tests: [{projectName: 'chromium', expectedStatus: 'passed', results: [{status}]}],
    }))}]}]};
}

test('only complete evidence for both selected scenarios can pass', () => {
    assert.equal(classifyReport(report(), 0).status, 'passed');
    assert.equal(classifyReport(report(['failed', 'passed']), 1).status, 'failed');
    assert.equal(classifyReport(report(['timedOut', 'passed']), 1).status, 'failed');
    assert.equal(classifyReport(report(['passed', 'skipped']), 0).status, 'not_run');
    assert.equal(classifyReport(report(['passed']), 0).status, 'not_run');
    assert.equal(classifyReport(report(), 1).status, 'not_run');
    assert.equal(classifyReport({errors: [{}], suites: report().suites}, 1).status, 'not_run');
    assert.equal(classifyReport(undefined, 0).status, 'not_run');
    const wrong = report();
    wrong.suites[0].suites[0].specs[1].tests[0].projectName = 'webkit';
    assert.equal(classifyReport(wrong, 0).status, 'not_run');
});

test('tool accepts only the fixed check and cannot change workspace or run arbitrary commands', async () => {
    let launches = 0;
    const handle = createHandler(() => { launches++; });
    for (const args of [{check_id: 'unknown'}, {command: 'echo arbitrary'}, {cwd: '/'}, {run_id: 42}]) {
        assert.throws(() => handle(request(args)), /Expected/);
    }
    assert.throws(() => handle(request({}, 'relative')), /Missing host workspace/);
    assert.equal(handle(request({run_id: 'unknown'})).status, 'not_run');
    await setImmediate();
    assert.equal(launches, 0);
});

test('run and status share one worker without starting a duplicate process', async () => {
    let finish, launches = 0;
    const handle = createHandler((root, runId) => {
        assert.equal(root, workspace);
        assert.notEqual(runId, '');
        launches++;
        return new Promise(resolve => { finish = resolve; });
    });
    const initial = handle(request({}));
    const status = request({run_id: initial.run_id});
    assert.equal(initial.status, 'running');
    assert.deepEqual(handle(request({})), initial);
    assert.deepEqual(handle(status), initial);
    assert.equal(handle(request({run_id: initial.run_id}, resolve(workspace, 'other'))).status, 'not_run');
    await setImmediate();
    assert.equal(launches, 1);
    finish({status: 'failed', exit_code: 1});
    await setImmediate();
    assert.deepEqual(handle(status), {check_id, run_id: initial.run_id, status: 'failed', exit_code: 1});
});

test('worker failure cannot produce a successful result', async () => {
    const handle = createHandler(() => { throw new Error('failed'); });
    const {run_id} = handle(request({}));
    await setImmediate();
    assert.equal(handle(request({run_id})).status, 'not_run');
});

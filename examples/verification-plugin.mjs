import {execFile} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {mkdtemp, realpath, stat, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {isAbsolute, join, relative, sep} from 'node:path';
import {createInterface} from 'node:readline';
import {pathToFileURL} from 'node:url';

const CHECK = 'browser-session-recovery';
const TOOL = 'verify_project_check';
const SPEC = 'tests/recovery.spec.mjs';
const CLI = 'node_modules/@playwright/test/cli.js';
const TITLES = [
    'reload restores a saved uninitialized conversation and draft without requiring a system message',
    'two windows reject conflicting saves and the stale window can export its unsent draft and attachment',
];
const TIMEOUT_MS = 90_000;

export function classifyReport(report, exitCode) {
    const cases = [];
    function visit(suite) {
        for (const spec of suite.specs ?? []) {
            for (const test of spec.tests ?? []) {
                cases.push({title: spec.title, project: test.projectName, expected: test.expectedStatus,
                    status: test.results?.length === 1 ? test.results[0].status : 'incomplete'});
            }
        }
        for (const child of suite.suites ?? []) visit(child);
    }
    for (const suite of report?.suites ?? []) visit(suite);
    const selected = TITLES.every(title => cases.filter(entry => entry.title === title).length === 1);
    if (cases.length !== TITLES.length || !selected || cases.some(entry => entry.project !== 'chromium')
        || report?.errors?.length) {
        return {status: 'not_run', reason: 'The expected Chromium scenarios did not run to completion.', cases};
    }
    if (cases.some(entry => ['failed', 'timedOut'].includes(entry.status))) return {status: 'failed', cases};
    if (exitCode === 0 && cases.every(entry => entry.expected === 'passed' && entry.status === 'passed')) {
        return {status: 'passed', cases};
    }
    return {status: 'not_run', reason: 'One or more required scenarios were skipped or incomplete.', cases};
}

async function requireFile(root, name) {
    const file = await realpath(join(root, name));
    const resolved = relative(root, file);
    if (isAbsolute(resolved) || resolved === '..' || resolved.startsWith('..' + sep) || !(await stat(file)).isFile()) {
        throw new Error('Invalid checkout file');
    }
    return file;
}

async function execute(workspace, runId) {
    let web, cli;
    try {
        const root = await realpath(workspace);
        web = join(root, 'web');
        cli = await requireFile(root, join('web', CLI));
        await requireFile(root, join('web', SPEC));
        await requireFile(root, 'web/playwright.config.mjs');
        await requireFile(root, 'web/pkg/i_bg.wasm');
    } catch {
        return {status: 'not_run', reason: 'Use a trusted i source checkout with built WASM and web dependencies.'};
    }
    const directory = await mkdtemp(join(tmpdir(), 'i-verify-'));
    const output = join(directory, 'test-results');
    const argv = [process.execPath, cli, 'test', SPEC, '--project=chromium', '--workers=1', '--retries=0',
        '--global-timeout=75000', '--reporter=json', '--trace=retain-on-failure', '--output', output,
        '--grep', `(?:${TITLES.join('|')})$`];
    const env = Object.fromEntries(['PATH', 'HOME', 'USERPROFILE', 'SystemRoot', 'TEMP', 'TMP', 'TMPDIR']
        .filter(key => process.env[key] !== undefined).map(key => [key, process.env[key]]));
    const startedAt = new Date().toISOString();
    const child = await new Promise(resolve => execFile(argv[0], argv.slice(1), {
        cwd: web, env, timeout: TIMEOUT_MS, maxBuffer: 2 * 1024 * 1024, windowsHide: true,
    }, (error, stdout, stderr) => resolve({error, stdout, stderr})));
    const reportPath = join(directory, 'playwright.json');
    const logPath = join(directory, 'stderr.log');
    const summaryPath = join(directory, 'summary.json');
    await writeFile(reportPath, child.stdout, {mode: 0o600});
    await writeFile(logPath, child.stderr, {mode: 0o600});
    let report;
    try { report = JSON.parse(child.stdout); } catch { /* No usable test report. */ }
    const exitCode = child.error ? (Number.isInteger(child.error.code) ? child.error.code : null) : 0;
    const result = child.error?.killed
        ? {status: 'not_run', reason: 'Runner terminated before returning complete evidence.'}
        : classifyReport(report, exitCode);
    const artifacts = {report: reportPath, stderr: logPath, summary: summaryPath};
    try { if ((await stat(output)).isDirectory()) artifacts.test_results = output; } catch { /* No test attachments. */ }
    const summary = {...result, check_id: CHECK, run_id: runId, argv, cwd: web, exit_code: exitCode,
        started_at: startedAt, completed_at: new Date().toISOString(), artifacts};
    await writeFile(summaryPath, JSON.stringify(summary, null, 2) + '\n', {mode: 0o600});
    return summary;
}

export function createHandler(run = execute) {
    let latest;
    return request => {
        const {event, workspace, data} = request;
        if (event !== 'tool' || data?.name !== TOOL) throw new Error('Unknown tool');
        const args = data.arguments;
        if (!args || args.check_id !== CHECK || Object.keys(args).some(key => !['check_id', 'run_id'].includes(key))
            || ('run_id' in args && typeof args.run_id !== 'string')) {
            throw new Error('Expected browser-session-recovery and an optional run_id');
        }
        if (typeof workspace !== 'string' || !isAbsolute(workspace)) throw new Error('Missing host workspace');
        if ('run_id' in args) {
            if (latest?.run_id === args.run_id && latest.workspace === workspace) return latest.result;
            return {check_id: CHECK, status: 'not_run', reason: 'Run not found in this worker.'};
        }
        if (latest?.result.status === 'running') {
            return latest.workspace === workspace ? latest.result
                : {check_id: CHECK, status: 'not_run', reason: 'This worker already has an active run.'};
        }
        const record = {workspace, run_id: randomUUID()};
        const identity = {check_id: CHECK, run_id: record.run_id};
        record.result = {...identity, status: 'running', retry_after_ms: 2_000};
        latest = record;
        void Promise.resolve().then(() => run(workspace, record.run_id)).then(
            result => { record.result = {...identity, ...result}; },
            () => { record.result = {...identity, status: 'not_run', reason: 'Could not retain verification evidence.'}; },
        );
        return record.result;
    };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    const handle = createHandler();
    for await (const line of createInterface({input: process.stdin, crlfDelay: Infinity})) {
        let request;
        try {
            request = JSON.parse(line);
            if (!Number.isSafeInteger(request?.id)) throw new Error('Invalid request ID');
            const result = handle(request);
            process.stdout.write(JSON.stringify({id: request.id, data: {output: JSON.stringify(result)}}) + '\n');
        } catch (error) {
            process.stdout.write(JSON.stringify({id: request?.id ?? null, error: error.message}) + '\n');
        }
    }
}

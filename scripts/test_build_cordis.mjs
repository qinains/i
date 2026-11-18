import assert from 'node:assert/strict';
import {test} from 'node:test';
import {spawnSync} from 'node:child_process';
import {cp, mkdtemp, readFile, rm, writeFile, mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {pathToFileURL, fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const script = join(root, 'scripts/build_cordis.mjs');
function run(args = [], cwd = root) {
  return spawnSync(process.execPath, [script, ...args], {cwd, encoding: 'utf8'});
}
async function scratch(t) {
  const path = await mkdtemp(join(tmpdir(), 'cordis-build-'));
  t.after(() => rm(path, {recursive: true, force: true}));
  return path;
}

test('rejects_modified_vendor_source', async t => {
  const dir = await scratch(t);
  await cp(join(root, 'vendor/cordis'), join(dir, 'vendor/cordis'), {recursive: true});
  const before = run(['--root', dir, '--check']);
  assert.equal(before.status, 0, before.stderr);
  await writeFile(join(dir, 'vendor/cordis/packages/core/src/context.ts'), 'tampered');
  const after = run(['--root', dir, '--check']);
  assert.notEqual(after.status, 0);
  assert.match(after.stderr, /hash mismatch.*context\.ts/u);
});

test('rejects_uninventoried_source', async t => {
  const dir = await scratch(t);
  await cp(join(root, 'vendor/cordis'), join(dir, 'vendor/cordis'), {recursive: true});
  await writeFile(join(dir, 'vendor/cordis/packages/core/src/unlisted.ts'), 'export const x = 1;');
  const result = run(['--root', dir, '--check']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /inventory.*unlisted\.ts/u);
});

test('bundle_has_no_bare_imports_and_is_reproducible', async t => {
  const dir = await scratch(t);
  const result = run(['--outdir', dir]);
  assert.equal(result.status, 0, result.stderr);
  const bytes = await readFile(join(dir, 'cordis.js'));
  const {Context, Service} = await import(pathToFileURL(join(dir, 'cordis.js')));
  const ctx = new Context();
  class Counter extends Service {
    constructor(ctx) { super(ctx, 'counter'); this.value = 0; }
    increase() { return this.ctx.effect(() => { this.value++; return () => this.value--; }); }
  }
  const provider = await ctx.use(Counter);
  const consumer = await ctx.inject(['counter'], ctx => { ctx.counter.increase(); });
  assert.equal(ctx.counter.value, 1);
  await consumer.dispose();
  assert.equal(ctx.counter.value, 0);
  await provider.dispose();
  assert.equal(run(['--outdir', dir]).status, 0);
  assert.deepEqual(await readFile(join(dir, 'cordis.js')), bytes);
});

test('includes_dependency_notices', async t => {
  const dir = await scratch(t);
  const result = run(['--outdir', dir]);
  assert.equal(result.status, 0, result.stderr);
  const notice = await readFile(join(dir, 'NOTICE'), 'utf8');
  assert.match(notice, /911b19dc13d7c48475ac015682be4d6677d31491/u);
  assert.match(notice, /cosmokit 1\.8\.1/u);
  assert.match(notice, /Permission is hereby granted/u);
  const upstreamLicense = await readFile(join(root, 'vendor/cordis/LICENSE'), 'utf8');
  const cosmokitLicense = await readFile(join(root, 'web/node_modules/cosmokit/LICENSE'), 'utf8');
  assert.ok(notice.includes(upstreamLicense.trim()));
  assert.ok(notice.includes(cosmokitLicense.trim()));
});

test('export_rebuilds_without_reference_checkout', {timeout: 120_000}, async t => {
  const dir = await scratch(t);
  await mkdir(join(dir, 'scripts'), {recursive: true});
  await mkdir(join(dir, 'web'), {recursive: true});
  for (const name of ['build_cordis.mjs', 'export_release.py']) {
    await cp(join(root, 'scripts', name), join(dir, 'scripts', name));
  }
  await cp(join(root, 'vendor/cordis'), join(dir, 'vendor/cordis'), {recursive: true});
  for (const name of ['package.json', 'package-lock.json']) await cp(join(root, 'web', name), join(dir, 'web', name));
  // Exercise the exporter rather than building directly from this checkout.
  const exported = spawnSync('python3', ['scripts/export_release.py'], {cwd: dir, encoding: 'utf8'});
  assert.equal(exported.status, 0, exported.stderr);
  const snapshot = resolve(dir, 'dist/release-source');
  const installed = spawnSync('npm', ['ci', '--prefix', 'web', '--no-audit', '--no-fund'], {cwd: snapshot, encoding: 'utf8', timeout: 90_000});
  assert.equal(installed.status, 0, installed.stderr);
  const built = spawnSync(process.execPath, ['scripts/build_cordis.mjs'], {cwd: snapshot, encoding: 'utf8'});
  assert.equal(built.status, 0, built.stderr);
  assert.deepEqual(await readFile(join(snapshot, 'web/plugins/vendor/cordis.js')), await readFile(join(root, 'web/plugins/vendor/cordis.js')));
});

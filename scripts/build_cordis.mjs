import {createHash} from 'node:crypto';
import {readFile, readdir, lstat, mkdir, writeFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {dirname, join, relative, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseArgs} from 'node:util';

const {values} = parseArgs({options: {
  root: {type: 'string', default: fileURLToPath(new URL('../', import.meta.url))},
  outdir: {type: 'string'}, check: {type: 'boolean', default: false},
}});
const root = resolve(values.root);
const vendor = join(root, 'vendor/cordis');
const commit = '911b19dc13d7c48475ac015682be4d6677d31491';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

async function files(path) {
  const stat = await lstat(path);
  if (stat.isSymbolicLink()) throw new Error(`Vendor symlink is not allowed: ${path}`);
  if (!stat.isDirectory()) return [path];
  const children = await readdir(path);
  return (await Promise.all(children.sort().map(name => files(join(path, name))))).flat();
}

async function verify() {
  const inventory = JSON.parse(await readFile(join(vendor, 'provenance.json'), 'utf8'));
  if (inventory.commit !== commit || inventory.repository !== 'https://github.com/cordiverse/cordis'
    || inventory.version !== '4.0.0-rc.13' || inventory.cosmokit !== '1.8.1') {
    throw new Error('Unexpected Cordis provenance pin');
  }
  const actual = (await files(vendor)).map(path => relative(vendor, path).replaceAll('\\', '/'))
    .filter(path => path !== 'provenance.json').sort();
  const expected = inventory.files.map(file => file.path).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`Vendor inventory mismatch: ${actual.filter(path => !expected.includes(path)).join(', ')}; missing: ${expected.filter(path => !actual.includes(path)).join(', ')}`);
  }
  for (const file of inventory.files) {
    if (hash(await readFile(join(vendor, file.path))) !== file.sha256) {
      throw new Error(`Vendor hash mismatch: ${file.path}`);
    }
  }
  const pkg = JSON.parse(await readFile(join(vendor, 'packages/core/package.json'), 'utf8'));
  if (pkg.name !== 'cordis' || pkg.version !== inventory.version) throw new Error('Vendored package metadata mismatch');
}

await verify();
if (values.check) {
  process.stdout.write(`Cordis ${commit}: source inventory verified\n`);
} else {
  const require = createRequire(join(root, 'web/package.json'));
  const {build} = require('esbuild');
  const cosmokit = JSON.parse(await readFile(join(root, 'web/node_modules/cosmokit/package.json'), 'utf8'));
  if (cosmokit.version !== '1.8.1') throw new Error('Expected locked cosmokit 1.8.1; run npm ci --prefix web');
  const output = resolve(values.outdir ?? join(root, 'web/plugins/vendor'));
  const result = await build({
    absWorkingDir: root, entryPoints: ['vendor/cordis/packages/core/src/index.ts'],
    outfile: join(output, 'cordis.js'), bundle: true, platform: 'browser', format: 'esm',
    target: 'es2022', nodePaths: [join(root, 'web/node_modules')],
    write: false, metafile: true, legalComments: 'none',
    banner: {js: `// Cordis ${commit}; licenses and provenance: ./NOTICE`},
  });
  for (const item of Object.values(result.metafile.outputs)) {
    if (item.imports.some(imported => imported.external)) throw new Error('Cordis bundle has external runtime imports');
  }
  const license = await readFile(join(vendor, 'LICENSE'), 'utf8');
  const dependencyLicense = await readFile(join(root, 'web/node_modules/cosmokit/LICENSE'), 'utf8');
  const notice = `Cordis 4.0.0-rc.13\nhttps://github.com/cordiverse/cordis\nCommit: ${commit}\n\n${license.trim()}\n\ncosmokit 1.8.1\n\n${dependencyLicense.trim()}\n`;
  await mkdir(output, {recursive: true});
  for (const file of result.outputFiles) {
    await mkdir(dirname(file.path), {recursive: true});
    await writeFile(file.path, file.contents);
  }
  await writeFile(join(output, 'NOTICE'), notice);
  process.stdout.write(`Cordis browser bundle and notices generated\n`);
}

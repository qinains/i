import {readFile,writeFile,readdir,copyFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const siteTool=fileURLToPath(new URL('../tools/site-tool/Cargo.toml',import.meta.url));
const toolTarget=fileURLToPath(new URL('../target/',import.meta.url));
execFileSync('cargo',['run','--quiet','--locked','--manifest-path',siteTool,'--target-dir',toolTarget,'--','translations','--check']);
execFileSync('cargo',['run','--quiet','--locked','--manifest-path',siteTool,'--target-dir',toolTarget,'--','brand-icons','--pwa']);
execFileSync('cargo',['run','--quiet','--locked','--manifest-path',siteTool,'--target-dir',toolTarget,'--','cors-extension']);
const root = new URL('../web/',import.meta.url);
await copyFile(new URL('../plugins/reflection-spec.json',root),new URL('reflection.json',root));
for(const name of ['LICENSE','THIRD_PARTY_LICENSES.txt'])await copyFile(new URL('../'+name,root),new URL(name==='LICENSE'?'LICENSE.txt':name,root));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
async function walk(dir) {
    const found = [];
    for (const item of await readdir(new URL(dir,root),{withFileTypes:true})) {
        const name = dir+item.name;
        if (item.isDirectory()) found.push(...await walk(name+'/'));
        else if (/\.(js|wasm)$/u.test(name)) found.push(name);
    }
    return found;
}
const pkg = await walk('pkg/');
for (const bridge of pkg.filter(p => p.endsWith('/web/platform.js'))) for(const dependency of ['sse.js','anthropic-sse.js','responses-sse.js','model-deadline.js','transport.js','connection-plugin.js','workspaces.js','memory-fs.js','opfs-locks.js']) await copyFile(new URL(dependency,root),new URL(bridge.replace(/platform.js$/u,dependency),root));
const modules=(await readdir(root)).filter(name=>name.endsWith('.js')&&!['sw.js','service-worker.js'].includes(name));
const extensionVersion=JSON.parse(await readFile(new URL('downloads/cors-helper.json',root),'utf8')).version;
const downloads=['cors-helper.json',...['chromium','firefox','safari'].map(browser=>`harness-cors-helper-${browser}-${extensionVersion}.zip`)].map(name=>'downloads/'+name);
const assets = ['index.html','commands.json','reflection.json','LICENSE.txt','THIRD_PARTY_LICENSES.txt','module-lexer.LICENSE','module-lexer.NOTICE',...modules,...await walk('state/'),...await walk('plugins/'),...await walk('plugin-pkg/'),'style.css','manifest.webmanifest','icons/icon.svg','icons/wordmark.svg','icons/icon-192.png','icons/icon-512.png',...await walk('pkg/')].sort();
assets.push(...downloads,'plugin-pkg/builtin.prompts-1.0.0.hplugin.zip');assets.sort();
const precache = await Promise.all(assets.map(async url => ({url,sha256:hash(await readFile(new URL(url,root)))})));
const source = await readFile(new URL('sw.js',root),'utf8');
const version = hash(JSON.stringify(precache)+source).slice(0,24);
await writeFile(new URL('service-worker.js',root),`self.HARNESS_BUILD = ${JSON.stringify({version,precache})};\n${source}`);
await writeFile(new URL('assets.json',root),JSON.stringify({version,precache,assets:[...assets,'service-worker.js']},null,2)+'\n');
process.stdout.write(`PWA ${version}: ${assets.length} verified static resources\n`);

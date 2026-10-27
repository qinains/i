import {readFile,writeFile,readdir,mkdir,copyFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const siteTool=fileURLToPath(new URL('../tools/site-tool/Cargo.toml',import.meta.url));
const toolTarget=fileURLToPath(new URL('../target/',import.meta.url));
execFileSync('cargo',['run','--quiet','--locked','--manifest-path',siteTool,'--target-dir',toolTarget,'--','translations','--check']);
execFileSync('cargo',['run','--quiet','--locked','--manifest-path',siteTool,'--target-dir',toolTarget,'--','cors-extension']);
const root = new URL('../web/',import.meta.url);
await copyFile(new URL('../plugins/reflection-spec.json',root),new URL('reflection.json',root));
for(const name of ['LICENSE','THIRD_PARTY_LICENSES.txt'])await copyFile(new URL('../'+name,root),new URL(name==='LICENSE'?'LICENSE.txt':name,root));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function crc(bytes) {
    let c = 0xffffffff;
    for (const byte of bytes) {c ^= byte; for (let bit=0; bit<8; bit++) c = (c >>> 1) ^ ((c & 1) ? 0xedb88320 : 0);}
    return (c ^ 0xffffffff) >>> 0;
}
function chunk(type,bytes) {
    const tag = Buffer.from(type), length = Buffer.alloc(4), checksum = Buffer.alloc(4);
    length.writeUInt32BE(bytes.length); checksum.writeUInt32BE(crc(Buffer.concat([tag,bytes])));
    return Buffer.concat([length,tag,bytes,checksum]);
}
// Rasterize the repo-native geometric SVG without platform-specific image tooling.
function icon(size) {
    const rows = Buffer.alloc((size*3+1)*size);
    for (let y=0; y<size; y++) for (let x=0; x<size; x++) {
        const sx = x*512/size, sy = y*512/size;
        const accent = (sx-256)**2+(sy-152)**2<24**2 || (sx>=224&&sx<288&&sy>=224&&sy<368) || (sx>=192&&sx<320&&sy>=368&&sy<400);
        const offset = y*(size*3+1)+1+x*3;
        rows.set(accent ? [34,211,238] : [15,23,42],offset);
    }
    const header = Buffer.alloc(13); header.writeUInt32BE(size); header.writeUInt32BE(size,4); header[8]=8; header[9]=2;
    return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]);
}
await mkdir(new URL('icons/',root),{recursive:true});
for (const size of [192,512]) await writeFile(new URL(`icons/icon-${size}.png`,root),icon(size));
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
const assets = ['index.html','commands.json','reflection.json','LICENSE.txt','THIRD_PARTY_LICENSES.txt','module-lexer.LICENSE','module-lexer.NOTICE',...modules,...await walk('state/'),...await walk('plugins/'),...await walk('plugin-pkg/'),'style.css','manifest.webmanifest','icons/icon.svg','icons/icon-192.png','icons/icon-512.png',...await walk('pkg/')].sort();
assets.push(...downloads,'plugin-pkg/builtin.prompts-1.0.0.hplugin.zip');assets.sort();
const precache = await Promise.all(assets.map(async url => ({url,sha256:hash(await readFile(new URL(url,root)))})));
const source = await readFile(new URL('sw.js',root),'utf8');
const version = hash(JSON.stringify(precache)+source).slice(0,24);
await writeFile(new URL('service-worker.js',root),`self.HARNESS_BUILD = ${JSON.stringify({version,precache})};\n${source}`);
await writeFile(new URL('assets.json',root),JSON.stringify({version,precache,assets:[...assets,'service-worker.js']},null,2)+'\n');
process.stdout.write(`PWA ${version}: ${assets.length} verified static resources\n`);

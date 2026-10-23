import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {readdir} from 'node:fs/promises';
import {once} from 'node:events';
import {fileURLToPath} from 'node:url';

test('static server serves both WASM packages with the streaming MIME type',async t=>{
    const child=spawn(process.execPath,[fileURLToPath(new URL('../serve.mjs',import.meta.url))],{env:{...process.env,PORT:'0'},stdio:['ignore','pipe','pipe']});
    t.after(async()=>{if(child.exitCode===null){const exited=once(child,'exit');child.kill();await exited;}});
    const origin=await new Promise((resolve,reject)=>{
        const timeout=setTimeout(()=>reject(new Error('Static server startup timed out')),5000);
        let output='';
        child.stdout.on('data',bytes=>{output+=bytes;const match=output.match(/http:\/\/127\.0\.0\.1:\d+/u);if(match){clearTimeout(timeout);resolve(match[0]);}});
        child.once('error',error=>{clearTimeout(timeout);reject(error);});
        child.once('exit',code=>{clearTimeout(timeout);reject(new Error(`Static server exited: ${code}`));});
    });
    for(const directory of ['pkg','plugin-pkg']){
        const files=(await readdir(new URL(`../${directory}/`,import.meta.url))).filter(name=>name.endsWith('.wasm'));
        assert.ok(files.length>0,'Build web assets before running tests');
        for(const file of files){
            const response=await fetch(`${origin}/${directory}/${file}`);
            assert.equal(response.status,200);assert.equal(response.headers.get('content-type'),'application/wasm');
            const module=await WebAssembly.compileStreaming(Promise.resolve(response));
            assert.ok(module instanceof WebAssembly.Module);
        }
    }
});

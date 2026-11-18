import assert from 'node:assert/strict';
import {test} from 'node:test';
import {access} from 'node:fs/promises';
import {build} from 'esbuild';
import {fileURLToPath} from 'node:url';

const obsolete=['cordis-runtime','index','example','agent-plugin','broker-plugin','cache-plugin','logger-plugin','manager-plugin','store-plugin','ui-plugin','worker-plugin'];
test('obsolete browser composition entrypoints are not shipped',async()=>{
    for(const name of obsolete){
        await assert.rejects(access(new URL(`../plugins/${name}.js`,import.meta.url)),{code:'ENOENT'});
    }
});
test('the production application bundles only the pinned upstream composition core',async()=>{
    const result=await build({absWorkingDir:fileURLToPath(new URL('../../',import.meta.url)),entryPoints:['web/app.js'],bundle:true,write:false,metafile:true,format:'esm',platform:'browser',external:['./pkg/*']});
    const inputs=Object.keys(result.metafile.inputs).map(path=>path.replaceAll('\\','/'));
    assert.ok(inputs.some(path=>path.endsWith('/plugins/vendor/cordis.js')));
    for(const name of obsolete)assert.ok(!inputs.some(path=>path.endsWith(`/plugins/${name}.js`)),name);
    for(const name of ['application','session-scope','task-controller','plugins/application','plugins/manager']){
        assert.ok(inputs.some(path=>path.endsWith(`/web/${name}.js`)||path===`web/${name}.js`),name);
    }
    assert.ok(!inputs.some(path=>path.endsWith('/state/recovery.js')));
});

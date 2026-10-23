import test from 'node:test';
import assert from 'node:assert/strict';
import {fileTree,viewState,persistentView,fileKind} from '../file-browser.js';
test('file tree separates mounts, reconstructs parents and sorts folders first',()=>{
    const roots=fileTree([{path:'z.txt',size:3},{path:'assets/js/app.js',size:7},{path:'tmp/a',size:1},{path:'/tmp/a',size:2},{path:'empty',directory:true}]);
    assert.deepEqual(roots.map(r=>r.path),['/workspace','/tmp']);assert.deepEqual(roots[0].children.map(r=>r.name),['assets','empty','tmp','z.txt']);
    assert.equal(roots[0].children[0].children[0].children[0].path,'assets/js/app.js');assert.equal(roots[1].children[0].path,'/tmp/a');
});
test('view state validates stored values and never persists scratch selection or trust',()=>{
    assert.deepEqual(viewState({mode:'bad',expanded:[{},'../bad'],selected:5}),{mode:'preview',expanded:['/workspace'],selected:'',open:false});
    const state=viewState({mode:'split',expanded:['/workspace','assets','/tmp/x'],selected:'/tmp/x.html',open:true,scripts:true});
    assert.deepEqual(persistentView(state),{mode:'split',expanded:['/workspace','assets'],selected:'',open:false});assert.equal('scripts' in state,false);
    assert.equal(fileKind('README.md'),'markdown');assert.equal(fileKind('pic.png'),'image');assert.equal(fileKind('app.rs'),'text');assert.equal(fileKind('pkg.wasm'),'binary');
});

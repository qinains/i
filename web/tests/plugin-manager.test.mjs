import {test} from 'node:test';
import assert from 'node:assert/strict';
import {publicToolName,grantFits} from '../plugins/manager.js';
test('plugin namespace and permission subset cannot enlarge a grant',async()=>{
    const name=await publicToolName('example.word-count','count_words');assert.match(name,/^p_count_words_[a-f0-9]{16}$/u);
    assert.notEqual(name,await publicToolName('other','count_words'));
    assert.equal(grantFits({workspace:'read',storage:false},{workspace:'read-write',storage:true}),true);
    assert.equal(grantFits({workspace:'read-write',storage:false},{workspace:'read',storage:false}),false);
    assert.equal(grantFits({workspace:'none',storage:true},{workspace:'none',storage:false}),false);
    assert.equal(grantFits({workspace:'none',storage:false,network:true},{workspace:'none',storage:false}),false);
});

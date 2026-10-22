import test from 'node:test';
import assert from 'node:assert/strict';
import {publishArchive} from '../download.js';
test('ZIP URL stays usable until its link is disposed', async () => {
    const link = {remove() {this.removed = true;}};
    const container = {ownerDocument:{createElement:() => link}, append() {}};
    const artifact = publishArchive({filename:'workspace.zip',bytes:new Uint8Array([80,75])},container);
    assert.equal(link.download,'workspace.zip');
    assert.deepEqual([...new Uint8Array(await (await fetch(artifact.url)).arrayBuffer())],[80,75]);
    artifact.dispose(); assert.equal(link.removed,true);
    await assert.rejects(fetch(artifact.url));
});

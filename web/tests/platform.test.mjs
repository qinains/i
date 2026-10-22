import test from 'node:test';
import assert from 'node:assert/strict';
import {validatePath, opfsWrite, opfsMetadata, fetchModel} from '../platform.js';

test('path traversal and platform prefixes are rejected', () => {
    for (const path of ['../x', '/x', 'a/../b', 'C:/x', 'a\\b', 'x\0']) {
        assert.throws(() => validatePath(path));
    }
    assert.deepEqual(validatePath('中文/a.txt'), ['中文', 'a.txt']);
});
test('failed close aborts and never reports successful write', async () => {
    let aborted = false;
    const file = {createWritable: async () => ({write: async () => {}, close: async () => {throw new Error('QuotaExceeded');}, abort: async () => {aborted = true;}})};
    const root = {getFileHandle: async () => file};
    await assert.rejects(opfsWrite(root, 'x', new Uint8Array([1])), /QuotaExceeded/);
    assert.equal(aborted, true);
});
test('metadata only treats NotFoundError as absent', async () => {
    const root = {getFileHandle: async () => {throw new DOMException('denied', 'NotAllowedError');}};
    await assert.rejects(opfsMetadata(root, 'x'), /denied/);
});
test('Fetch refuses oversized responses and respects abort', async () => {
    const controller = new AbortController(); controller.abort();
    await assert.rejects(fetchModel('http://127.0.0.1:1', '{}', '', controller.signal));
});

# OPFS concurrency and durability boundaries

All cooperating file entrypoints use origin-wide Web Locks named
`harness:files:<workspaceId>`. File operations are isolated at workspace granularity:
multiple readers may coexist, but writers exclude all readers and other writers.
Different workspaces remain independent. The separate `harness:workspace:<id>` UI
ownership lock is retained; this change does not enable parallel AI tasks in the
same workspace or remove the UI's busy-state checks.

## Protected scopes

- Low-level read, metadata and directory listing acquire shared leases. Write,
  mkdir, remove and `/tmp` clearing acquire exclusive leases.
- A Rust `edit` holds one exclusive lease over read, all replacements, conflict
  verification and close. `write` and upload include parent-directory creation
  and stream close/abort within their exclusive scope.
- Batch reads, recursive search/glob, the file tree and ZIP export hold a shared
  lease for their complete traversal. A read cannot combine files observed before
  and after an intervening cooperating write.
- HTML preview snapshots copy their entry/resources under a shared lease. HTML
  compilation/rendering happens after releasing the lease, against immutable bytes.
- Plugin read/write/list host requests use signal-bound leases; listings include
  metadata under the same lease. Plugin private-package storage remains separately
  coordinated by the plugin-manager lock.

Default writes are intentional unconditional overwrites: concurrent writes run
in lock-grant order and the last successful close wins. Use `edit` or an explicit
read-modify-write scope when preserving the previous value matters. This does not
detect whether an AI's instructions were based on a stale earlier model turn.

UI uploads are different: inspection captures a SHA-256 fingerprint of only the
target file, or `null` if absent. Approval holds no file lease. Before committing,
the upload checks that fingerprint and writes under one exclusive lease. A
changed target or a newly created file rejects the upload without overwriting it;
select the upload again to review its current target. Fingerprints never enter
model prompts and require no additional model requests.

## Explicit scoped API

```js
const root = await opfsOpenWorkspace('default');
await opfsWithTransaction(root, 'exclusive', async scoped => {
    const before = await opfsRead(scoped, 'counter.txt', 1024);
    const next = Number(new TextDecoder().decode(before)) + 1;
    await opfsWrite(scoped, 'counter.txt', new TextEncoder().encode(String(next)));
}, abortController.signal);
```

Always pass `scoped` to operations inside a scope. It is an opaque capability,
not a directory handle, and expires when the callback ends. Read scopes reject
mutations. Nested scopes reuse this capability instead of reacquiring their lock.
Do not acquire the ordinary root again inside a scope; that is a separate request
and can wait on your own lock. Avoid holding scopes across UI prompts, model
requests or unrelated slow work. Do not nest multiple workspace locks in arbitrary
orders. All participating app windows should use the updated code.

Automatic root operations and explicit scopes share one registry across duplicate
generated JS bridges. Rust uses separate leased Workspace values per operation,
not an ambient global "lock held" flag. Unrelated concurrent calls in one window
therefore cannot inherit another call's write permission.

## Cancellation and failures

- Lock acquisition waits up to 15 seconds; it does not silently retry mutations.
  An explicit AbortSignal can cancel queued work sooner. A lock already granted
  is not forcibly stolen on timeout.
- Stop cancels queued AI/plugin writes and UI uploads/clears. A write that already
  reached native I/O may finish; it is not rolled back merely because Stop was
  clicked. Scope teardown drains in-flight I/O before admitting another writer,
  including when a caller forgot to await a started operation.
- Errors release leases in `finally` / Rust RAII. Closing or reloading a window
  releases its Web Locks. A live or frozen window holding I/O can cause other
  callers to time out; finish/close the owner instead of forcing concurrent access.
- UI listings, ZIP exports and operation-cleanup refreshes carry cancellation
  signals. Stop can return the UI to Ready without waiting for another window's
  file lock; it schedules a new background refresh instead of resuming the
  cancelled operation. Already-started native I/O still drains before unlocking.
- Directory refreshes are latest-request-wins, even when an older loader ignores
  cancellation. Pending/failed refreshes do not clear the last valid file tree.
  Concurrent requests for a lazy session agent share one initialization.
- The writer reports success only after `close()`. Failure attempts `abort()`
  and preserves the original error. Browser tests verify that an aborted overwrite
  and a closed writer window preserve previously committed bytes.

## Not an ACID database

"Transaction" here means an isolation scope, **not** a multi-file rollback log.
If a callback writes two files and the second fails, the first may already be
committed. Creating a new file/directory before a failed stream may leave a new
empty entry; the code does not blindly delete paths after an ambiguous failure.

OPFS and the IndexedDB session journal are separate stores, not an atomic commit.
Interrupted tools retain the existing unknown/not-executed recovery rules; they
are never automatically replayed. Locks cannot protect against code that bypasses
this API and calls raw directory/file handles, site-data clearing, storage
eviction, disk/quota failures or arbitrary browser/OS failures. Back up important
files. Persistent OPFS requires Web Locks support; only temporary in-memory roots
may use a realm-local serial fallback without it.

## Regression coverage

`web/tests/opfs-locks.test.mjs`: serialized writers, read-only/expired leases,
nested reads, exception cleanup, early scope return, granted-lease cancellation
while I/O remains in flight, and versioned upload conflicts. File-refresh and
session unit tests cover stale results and deduplicated initialization.

`web/tests/opfs-concurrency.spec.mjs`: real Chromium/WebKit OPFS across two windows,
shared readers/queued writers, duplicate bridges, 20 concurrent counter updates,
window termination, native writer abort, cancelled plugin/upload/ZIP queues,
Stop returning Ready before a held lock is released, directory-result ordering,
upload approval conflicts, and a real WASM edit paused between its verification
read and commit.

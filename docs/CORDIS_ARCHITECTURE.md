# Cordis composition architecture

The browser uses the actual upstream Cordis core; native Rust implements the selected core semantics.
This is not a port of Cordis's Node loader, HMR or every optional package.

## Reference and build

- Pinned commit: `911b19dc13d7c48475ac015682be4d6677d31491`.
- Core: `4.0.0-rc.13`; cosmokit: `1.8.1`.
- Unmodified source, MIT license and SHA-256 inventory: `vendor/cordis/`.
- Build: `npm ci --prefix web && sh scripts/build_web.sh`.
- Provenance check: `node scripts/build_cordis.mjs --check`.
- Generated browser ESM and notices: `web/plugins/vendor/`; offline PWA inventory includes both.
- No runtime CDN, build-time Git fetch, reference-checkout dependency or Node prerequisite for native Cargo builds.

## One authority per host

| Host | Composition owner | Domain and execution adapters |
| --- | --- | --- |
| Browser | Actual upstream Context, Service, Fiber and effects | BrowserAgent WASM, real OPFS, package manager/broker/guest workers |
| Native | `src/module/kernel/`, mounted by `src/application/native.rs` | Native provider/workspace, restored `plugins/runtime`, subprocess workers |

Component definitions, Fiber instances, service bindings and package admission entries have different
identities. Package catalogs and worker protocols are not additional application activation registries.
The old module registry, standalone lifecycle/resolver demos, handwritten browser kernel and demo
wrappers have been removed. There is no old-API compatibility facade.

## Browser ownership

`web/application.js` creates one upstream root. The real application in `web/app.js` uses it for:

- A lazily mounted package-manager boundary with owned store acquisition and operation draining.
- One explicit shared WorkspaceService per workspace.
- Session components with per-key isolated record, model and adapter bindings.
- Dependent operation components for runs and asynchronous file operations.

Model replacement requests cancellation and retires the old activation before acquiring its replacement.
An admitted task retains its old adapter until its final checkpoint/file call completes. A queued task
that never activated rejects rather than escaping onto a later model. Existing conversation attachment
checks prohibit implicitly switching API accounts, connections or models.

The WASM adapter executes the shared Agent loop and domain tools, not a second Rust composition graph.
Successful file writes and durable history are emissions: unloading does not undo them.
`web/task-controller.js` owns task admission and phases, the approval and cancellation state, and
queue dispatch. The DOM calls it and renders its state; session persistence remains with `SessionController`.

## Native ownership

`AgentApplication` supplies scoped provider/workspace access to `ApplicationHost` hooks. The actual
CLI routes provider completion, cache/compaction, plugin approval/tool hooks and checkpoint persistence
through those services. Each run is an instance of one component definition.

Configured native sessions atomically save a tool-batch journal before dispatch, mark calls started
before entering hooks/approval/tools, and persist finished results before the batch checkpoint.
Interrupted started calls recover as unknown outcomes, never automatic replays. Browser and native
adapters and browser startup all use `src/agent/recovery.rs`; browser startup enters through the WASM
bridge. Native CLI runs hold a stable per-session file lock across reads and checkpoints. Journals bypass mutable
save hooks; normal complete-history checkpoints retain the existing hook contract.

`ServiceAccess.call` and `call_async` retain committed bindings without exposing raw Arc handles to
normal consumers. A service resolves dependencies at its definition site; resources created for a caller
use that caller's effect scope. Metadata uses typed provider-defined merging, not ambient thread/task state.

`Context.extend` retains ownership and service realms. `isolate` changes one service key's realm;
it is not authorization and does not clone the entire service universe.

## Transitions and cleanup

- States: PENDING, LOADING, ACTIVE, FAILED, UNLOADING, DISPOSED.
- Independent Fibers progress independently; one Fiber reconciles its latest requested target.
- Setup, rollback and teardown retain the exact admitted binding records.
- Owner provisions remain readable by owner/child disposers after public withdrawal, until owned
  cleanup completes. Manual explicit withdrawal is a separate operation, not an owner shutdown.
- Withdrawal stops new admission, then drains old consumers, including publicly unregistered consumers.
- Provider cleanup follows dependent cleanup. Sibling bindings need not wait for unrelated retirements.
- Independent top-level inverses start in reverse order and drain concurrently; explicit groups serialize.
- A late disposer is collected; partial setup failure rolls back acquired effects.
- FAILED does not retry on ordinary dependency refresh. Explicit update or definition replacement recovers.
- Emit, bail, serial, parallel and continuation-based waterfall are distinct event algorithms.

## Terminal shutdown

Application shutdown is separate from upstream's reusable root restart:

1. Reject new work and request cancellation.
2. Wait for admitted operations and safe persistence/checkpoint boundaries.
3. Drain dependent activations and release their resources.
4. Close persistent stores and return one shared shutdown report.

Bounded waits report retained resources; they never justify freeing resources still in use.
Cleanup continues independently when a caller abandons its waiter. Non-yielding trusted native code
cannot be forcibly preempted. Existing untrusted worker/process hard termination remains intact.
Hooks request ancestor shutdown without awaiting themselves. BFCache suspension cancels only;
non-persisted page teardown requests terminal shutdown.

## Durable tools and security

`read` accepts `files` ranges; `edit` accepts a path and an `edits` batch. Legacy top-level read/edit
forms and contradictory mixed arguments reject before committing a write. Batch atomicity, CRLF,
output budgets, approvals, conditional writes, journals and unknown-result/no-replay recovery remain required.

Plugin packages are validated before installation and again before use. Workspace grants, immutable
snapshots, OPFS locks and worker deadlines remain execution authorization boundaries. Guests never
receive a host Context. Disabling or updating a plugin retires future tool snapshots; a held snapshot
pins the admitted invocation through completion.

Invalid persisted conversations are retained in storage and remain available through raw store export;
a failed validation does not authorize deletion.

## Verification

Shared fixtures under `tests/fixtures/cordis/` execute the real upstream core and native kernel using
named barriers. Adapters preserve event ordering and error/state outcomes; watchdogs diagnose stalls,
not schedule them. Focused application tests use real filesystem/WASM/OPFS and guest boundaries.

See [runtime evidence and limits](cordis-runtime.md) for source audit and verification status.
Local macOS checks do not certify Linux/Windows runtime behavior.

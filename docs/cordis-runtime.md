# Cordis-inspired runtime: implementation and limits

This project adapts Cordis's composition semantics to Rust/WASM and browser ES modules.
It does not implement the upstream TypeScript API or claim full feature parity.

## Upstream source audit (2026-11-14, pre-migration baseline)

This audit records the pre-migration implementation, not a conformance certificate. The migration
status below distinguishes subsequently implemented contracts from outstanding gaps. API naming
alone is not evidence of the full context paradigm or spatiotemporal composability.

### Evidence baseline

- Upstream: `cordiverse/cordis`, commit `911b19dc13d7c48475ac015682be4d6677d31491`,
  core package version `4.0.0-rc.13`. This is the behavioral reference, rather than an unpinned main.
- Read the core Context, Fiber, Registry, Reflect, Service, Events and traceable-context implementation;
  corresponding effect, service-disposal, isolation, shadow, replacement and update tests; loader entry,
  group and realm reconciliation; timer-service effect ownership; and HMR transaction boundaries.
- Read the linked paper's context/effect/coeffect definitions, implementation correspondence and system
  boundary discussion. The paper explains the discipline; where its pseudocode differs from the pinned
  implementation, use the source and executable tests for exact runtime behavior.
- In an isolated reference checkout, installed locked dependencies without lifecycle build scripts,
  built using upstream Yakumo/esbuild, then ran:

  ```sh
  node node_modules/vitest/vitest.mjs run packages/core/tests packages/loader/tests packages/hmr/tests packages/timer/tests --reporter=dot
  ```

  Result: 35 files passed, 495 tests passed, 2 upstream tests skipped. Log:
  `target/cordis-upstream-semantics-test.log`. The reference checkout is under
  `target/reference/cordis-2026-11-14`; it is not a new production dependency.
- Bundled the unmodified core for `platform=browser` and executed a Chromium probe: a consumer invoked
  a Counter service that created an effect through `this.ctx`; disposing only the consumer changed the
  counter from 1 back to 0 while leaving the provider installed. Browser core reuse is technically
  feasible for that scenario; this does not certify the whole application integration.

### What the source actually requires

| Concern | Pinned upstream behavior | Current local gap |
| --- | --- | --- |
| Effect mediation | Provision, listeners, component instantiation and service-created registrations reduce to an owned effect runner. Nested effects compose their inverses. | Context owns direct service/listener registrations, but Fiber and EffectTracker remain separate helpers; service-created effects are not automatically attributed to their consumer. |
| Component identity | A component definition has a runtime; each `use` creates an independent Fiber with its own config, identity and lifecycle. One definition may have multiple instances. | The application registry is organized around unique plugin names and one activation per entry. The standalone local Fiber is not that registry's activation state machine. |
| Dependency view | Each activation commits actual binding records. Its old binding view remains accessible during setup rollback and teardown. | Context lookup reads the current shared service table, not the activation's committed view. A replacement can become visible to the old consumer's unload hook. |
| Provider drain | A provider stops admitting new dependents, then waits for consumers of each retired binding to finish, including consumers already removed from the registry. | Reverse activation ordering and service revisions do not provide a per-binding committed-view/drain protocol. |
| Async reconciliation | Each Fiber has an inertial transition and a latest target; a superseded load rolls back before the next activation starts. Independent fibers need not share one global busy state. | Rust hooks are synchronous; the browser registry rejects overlapping mutations globally. These are local constraints, not the upstream transition model. |
| Isolation and interception | `isolate` redirects selected service keys through realm identities; `intercept` carries scoped access metadata. Neither means cloning the entire service universe. | Separate roots isolate whole workspaces, but there is no equivalent per-service realm or def-site/use-site mediation. |
| Cleanup ordering | Independent top-level effects start in reverse registration order and drain concurrently. Explicit effect groups serialize dependent inverses; provider dependencies impose additional barriers. | Local cleanup is globally sequential within a scope. LIFO alone is not the complete upstream ordering guarantee. |
| Failure recovery | A failed Fiber remains failed across ordinary dependency refresh; explicit update or component replacement can recover it. | Local failed signatures can retry after dependency changes. |
| Events | `waterfall` is an around-middleware continuation, not a value-folding pipeline. Event modes have distinct short-circuit, async and error behavior. | The core Context bus mainly provides synchronous emit; host/subprocess hooks are a separate mechanism. |

Sources:
[Fiber](https://github.com/cordiverse/cordis/blob/911b19dc13d7c48475ac015682be4d6677d31491/packages/core/src/fiber.ts),
[binding resolution](https://github.com/cordiverse/cordis/blob/911b19dc13d7c48475ac015682be4d6677d31491/packages/core/src/reflect.ts),
[service call mediation](https://github.com/cordiverse/cordis/blob/911b19dc13d7c48475ac015682be4d6677d31491/packages/core/src/utils.ts),
[component registry](https://github.com/cordiverse/cordis/blob/911b19dc13d7c48475ac015682be4d6677d31491/packages/core/src/registry.ts),
[events](https://github.com/cordiverse/cordis/blob/911b19dc13d7c48475ac015682be4d6677d31491/packages/core/src/events.ts),
[paper](https://arxiv.org/abs/2608.25512v1).

### Reproduced behavioral difference

For a consumer loaded against `foo.version = 1`, replace the binding with version 2 and record the
consumer's load/unload reads:

```text
Pinned upstream: load(1), unload(1), load(2)
Current browser: load(1), unload(2), load(2)
```

The local probe waits for `registry.settled()` as required by its existing contract. The upstream probe
withdraws the old provision, installs a fresh provision, then awaits retirement and the consumer. This
is a semantic mismatch, not a request to preserve either API's syntax. Its upstream regression is in
`packages/core/tests/service-dispose.spec.ts` (old-binding withdrawal and next-load separation).

### Consequences for the next migration design

1. Start with differential behavior tests for committed views, consumer-owned service effects, binding
   retirement, in-flight supersession, multiple component instances and explicit failed-state recovery.
   Existing green local tests are a baseline, not evidence that these upstream invariants already hold.
2. Evaluate reusing the pinned browser-compatible upstream core instead of maintaining a second
   handwritten JavaScript kernel. Rust needs an explicit semantic port, with typed accessors/call scopes
   standing in for JavaScript proxy mediation; do not add an embedded JS runtime merely for API parity.
3. Keep component definitions, Fiber instances and declarative loader entries as distinct layers.
   "One kernel" does not mean collapsing these identities into one flat plugin-name map. Application,
   workspace, session and run are business ownership scopes, not substitutes for service isolation.
4. Deepen actual Agent/provider/workspace integration through that kernel, then remove obsolete local
   registries, placeholders and old tool argument formats after auditing their callers.
5. Keep process/WASM execution and approval boundaries. Framework resource release is not automatic
   reversal of emitted model requests or user file edits. An application-level terminal shutdown must
   also be distinguished from the upstream root Fiber's reusable restart/dispose behavior.

The audit originally changed documentation only. Implementation has since started under the approved
design (`docs/superpowers/specs/2026-11-14-upstream-cordis-alignment-design.md`) and
plan (`docs/superpowers/plans/2026-11-14-upstream-cordis-alignment.md`). The gap table and trace above describe
the pre-migration implementation, not a claim that the newly staged kernel has the same behavior.

## Implemented composition contracts

Browser composition uses the actual pinned upstream Context/Service/Fiber/effect implementation,
not a handwritten approximation. Native-only `src/module/kernel/` implements the corresponding
selected contracts through typed ServiceKey/ServiceAccess and explicit call scopes.

The shared fixture suite executes eighteen scenarios in both implementations, spanning all ten
required categories: committed old-binding teardown, pending arrival/loss and asynchronous readiness,
late setup/rollback, already-unregistered consumers, transitive/per-binding drain, consumer-owned grouped
effects, concurrent/latest-target transitions, multiple instances and per-key realms, failed-state
recovery, five event modes and definition replacement. Scoped interception additionally verifies
definition-site dependencies and caller metadata. Rust adds parser rejection checks and focused
cleanup/error/cancellation/ownership regressions. This is case coverage, not full optional-package parity.

### Actual browser application

`web/application.js` owns a single upstream root. Package-manager composition is a child boundary,
with real asynchronous store acquisition and owned manager calls. Workspaces share explicit services;
sessions isolate record/model/adapter bindings. Runs and asynchronous file operations are dependent
components, not awaited whole-run setup hooks. Their cancellation/join inverses drain before adapter
close/free. Deferred drafts flush even when no adapter was acquired. Startup Ready is published only
after initial adapter acquisition and initialization writes complete.

BrowserAgent supplies the shared Agent loop, provider conversion, history validation and workspace
domain capabilities without constructing a second Rust registry. Explicit async close drains operations
and checkpoints, clears credentials/plugin callbacks and retains exportable history.

Application shutdown is terminal, joins repeated calls and closes state only after checkpoint drains.
Its bounded observer can report retained resources while independently owned cleanup continues.
BFCache suspension only cancels; non-persisted page teardown requests shutdown without self-await.

### Actual native application

`src/application/native.rs` owns the native graph. `src/main.rs` uses ApplicationHost and typed
provider/workspace Services for real provider, compaction/cache, approvals, tool dispatch, checkpoint
and finish hooks. Restored `plugins/runtime` workers remain execution/protocol adapters, not another
activation authority. Recovery provenance is preserved under `.tasks/recovery/`.

A conversation records application identity and provider generation. Replacement cancels/drains old
runs and requires explicit reattachment before sending existing history to a new provider. Borrowed
service access keeps the committed binding without holding locks across user hooks or awaits.
Successful writes and persisted history are durable emissions, never cleanup inverses.

### Native adaptations

- Concrete Rust names/types differ from JavaScript Proxy mediation, while committed views,
  definition/use-site scope and caller-owned effects remain explicit.
- Typed metadata merging is provider-defined. Invalid metadata is an error rather than silently ignored.
- Shared event outcomes preserve individual parallel errors; AggregateError container labeling is a
  diagnostic adaptation, not a change to listener ordering or state.
- Upstream logs cleanup errors. Application shutdown additionally returns a structured report.
- Root restart is reusable; application shutdown is terminal. Bounded drain retains in-use resources.
- Arbitrary non-yielding trusted native code cannot be preempted. Existing untrusted worker termination
  is separate from cooperative graph cancellation.

### Removed obsolete interfaces

The handwritten browser kernel, demo wrappers, old CoreModule/ModuleRegistry, standalone lifecycle/
resolver helpers, placeholder Rust services and unused alternate CLI are removed. Benchmark callers
use the production kernel. The real CLI, configuration/workers, broker, package catalog, cache,
compaction and console implementations remain.

Workspace read/edit use only `files`/`edits`. Legacy and mixed forms reject. Unknown persisted records
are not deleted on validation failure and remain available through raw storage reads/serialization.

## Verification

```sh
cargo fmt --all --check
cargo test --workspace --all-targets
cargo test --workspace --all-targets --features bundled,packaging
cargo test --workspace --doc
cargo clippy --workspace --all-targets --features bundled,packaging -- -D warnings
cargo check --no-default-features -p i
cargo check --target wasm32-unknown-unknown --no-default-features --features wasm
npm ci --prefix web
sh scripts/build_web.sh
sh scripts/build_plugin_fixtures.sh
npm test --prefix web
npm run test:browser --prefix web
node --test scripts/test_build_cordis.mjs
node --test server/tests/*.test.mjs
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Final-source verification completed on 2026-11-14: default native workspace/all-target tests
372 passed / 1 existing ignored; bundled+packaging 382 passed / 1 existing ignored; doctests 1 passed.
Strict Clippy, formatting, minimal and WASM checks passed. Node 150/150, relay 7/7, build/export
5/5 and script 8/8 tests passed. One uninterrupted Chromium/WebKit run passed 288/288 in 18.8 minutes,
with no blanket retries or new skips. Historical failed runs remain in the execution ledger.

One independent whole-change review found two important issues (own-provision teardown and native
per-tool recovery) and one minor listener-resource issue. All three were reproduced and fixed in one
regression-tested pass; this is not a claim of a second reviewer approval. Native and browser now
share `src/agent/recovery.rs`; native journals retain unknown outcomes rather than replaying tools.

Independent source export regenerated the identical actual core/NOTICE/WASM/PWA (version
`88828c895325facbc7a0476b`, 71 static resources), with no reference checkout or generated input.
Fresh macOS ARM64 gzip/xz packages passed hashes, relocation, actual bundled workers/console and
TLS checks. Exact commands, inventory hashes, package provenance and review resolution are in
`.superpowers/sdd/2026-11-14-upstream-cordis-alignment/` in the original development workspace.

Ignored browser bundles/WASM from an older source revision do not prove current source builds.
Rebuild before browser validation; exports must regenerate without `target/reference`.
Additional platform verification on 2026-11-14 ran the real native matrix in an isolated Ubuntu
24.04.5 ARM64 guest: default 372 passed / 1 existing ignored, bundled+packaging 382 / 1,
doctests 1 passed, strict Clippy/minimal/format checks passed, and real release gzip/xz,
console/setup/TLS/relocation checks passed. CI dev/test DWARF is disabled: Linux symbol-heavy
debug executables exceeded the installer's preserved 64 MiB limit; no assertions were disabled.
Linux packages are in `dist/linux-arm64-verified/` in the development workspace.

This does not certify Windows, Linux x86_64 or macOS Intel execution. No real model credentials or
Developer ID/notarization setup were supplied. The current macOS artifact has a valid ad-hoc
signature but is rejected by Gatekeeper; it is not a signed/notarized distribution.

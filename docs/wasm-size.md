# Browser WASM size

Default web builds use the existing `min-size` Cargo profile for both the agent
and the plugin-package validator. It enables `opt-level = "z"`, fat LTO, one
codegen unit, aborting panics and stripped symbols. Native default profiles and
browser features are unchanged. `wasm-opt` is not required or downloaded by the
normal build.

## Measured 2026-10-23

Sizes are bytes, using the same source and toolchain before/after changing the
build profile from default release to min-size:

| Module | Release raw | Min-size raw | Reduction | Min-size gzip | Min-size Brotli |
| --- | ---: | ---: | ---: | ---: | ---: |
| `pkg/i_bg.wasm` | 2,146,662 | 1,339,525 | 37.6% | 487,806 | 382,707 |
| `plugin-pkg/validator_bg.wasm` | 891,161 | 609,321 | 31.6% | 219,843 | 180,862 |

After adding explicit OPFS concurrency scopes later the same day, the agent is
1,341,287 bytes raw / 488,708 gzip / 383,833 Brotli (+1,762 raw bytes over the
initial min-size result); the validator is unchanged. The table above preserves
the same-source profile comparison rather than attributing feature changes to
compiler optimization.

The subsequent cancellable listing/ZIP and version-checked upload follow-up is
1,342,084 bytes raw / 489,268 gzip / 383,830 Brotli. The validator is unchanged.

Compression figures use gzip level 9 and Brotli quality 11. These are measured
compressed payload sizes, **not** the current development server's transfer
sizes: `web/serve.mjs` sends raw WASM with `application/wasm`. A deployment server
must separately enable gzip/Brotli and preserve the MIME type. PWA caches retain
the browser-decoded modules; no persistent workspace data is changed.

Reproduce:

```sh
./scripts/build_web.sh
node scripts/measure_wasm.mjs
```

For a comparison build, use `WASM_PROFILE=release ./scripts/build_web.sh`, then
restore the normal build before publishing. Custom measurement paths can be
passed to `measure_wasm.mjs`. Building for size can trade execution throughput
for code size; functional browser tests are not a throughput benchmark.

An additional Binaryen pass was attempted in a separate `/tmp` build. The
wasm-pack Binaryen 117 download failed, so there is no measured or claimed
`wasm-opt` saving in these results. Further reductions would require measuring
that pass or changing dependency/feature boundaries; no Unicode search,
plugin validation, ZIP, OPFS or model functionality was removed here.

## Official globset dependency, 2026-10-26

The local feature-only globset patch was removed in favor of checksum-locked,
unmodified crates.io globset 0.4.20. No glob syntax or matching implementation was
rewritten. Local min-size artifacts measured before/after the switch:

| Agent module | Raw | gzip level 9 | Brotli quality 11 |
| --- | ---: | ---: | ---: |
| Before, local feature patch | 1,362,270 | 493,773 | 388,118 |
| After, official crates.io dependency | 1,362,527 | 493,668 | 387,602 |

The raw module grew 257 bytes (about 0.019%); compressed payloads were slightly
smaller. This is a local artifact comparison, not a general performance claim.
Third-party notices remain in the distribution; removing the source copy does not
remove the dependency from compiled code. Download sizes depend on server
compression. Historical measurements above are retained as dated results.

The offline native literal-search fixture (6.4 MB, seven runs per binary,
including process startup and file reads) had medians of 33.781 ms before and
33.843 ms after. This small, noisy fixture showed no material regression; it
does not establish performance for all glob or regex workloads. Reproduce with
`cargo run --locked --manifest-path tools/repo-tool/Cargo.toml -- bench-search BEFORE AFTER --runs 7`.

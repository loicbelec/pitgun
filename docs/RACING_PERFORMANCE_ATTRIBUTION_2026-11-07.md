# Racing performance attribution — 7 November 2026

Follow-up: the [signed Model 0.14 replay and terminal-retention correction](RACING_SIGNED_TERMINAL_RETENTION_2026-11-07.md)
show a measured WASM memory gain with identical output/evidence. The V9 experiment
below remains the historical initial screen; its scope and metrics are unchanged.

The problem is mixed: the Racing domain already has a substantial memory cost
without WASM, and WASM adds execution/binding overhead in this controlled case.
This is evidence for improving retention and transport before contemplating a
rewrite of the generic framework. It is not proof that the physics kernel alone,
the generic runtime, or the browser is the sole bottleneck.

Tracking: [framework #430](https://github.com/loicbelec/pitgun/issues/430) for
retention/terminal output; [game #315](https://github.com/loicbelec/pitgun-game/issues/315)
for live browser/host/archive memory. This initial screen delivers no optimization
or release waiver; the separate signed follow-up above contains the retained fix.

## Controlled experiment

Nine sequential fresh-process trials, three per mode in rotating order, use the
same public Monza53 input: ten cars, 5,787 spatial samples, 5 Hz player telemetry,
37,857 resulting frames. Catalogue 1.8.0, initial request and thermal profile are
identical. Native and rebuilt WASM use framework base `4c2212b`, Rust 1.89.0,
release optimization and pinned wasm-bindgen 0.2.106, on Apple M4 Pro / 24 GiB.
Node is recorded in the
[compact evidence](qa/racing-performance-attribution-2026-11-07.json).

**Important scope:** this exercises the existing local component/thermal V9
entrypoint, not the governed signed Model 0.14 timeline/application path currently
configured by the game. It isolates a shared Racing pipeline. The separate live
signed-memory observations concern a different career/setup and must not be
combined into an A/B comparison. This is not an Engineer or Grand Prix benchmark.

| Median of three | Native typed Racing API | Native JSON pull API | WASM JSON pull API, Node |
| --- | ---: | ---: | ---: |
| Time inside framework APIs, including setup | 5.122 s | 5.255 s | 10.605 s |
| Host JSON parsing | none | 0.912 s | 1.067 s |
| Whole measured call sequence | 5.130 s | 6.400 s | 11.744 s |
| Whole-process maximum RSS | 941.8 MiB | 1,612.6 MiB | 1,028.0 MiB |

These are separate medians; columns are not additive component accounting. RSS
includes ordinary host parsing where applicable, not only a Solver allocation.
The typed runner deliberately avoids converting the full result into a JSON
graph. OS/private footprint, RSS and WASM linear capacity are different metrics.
The WASM linear memory ends at 827.8 MiB; this is capacity, not resident memory.
There is no forced GC, browser, UI, gzip, IndexedDB or signature verification.

All nine runs have identical standings/times, per-lap milliseconds and frame
count. This does not claim bit-identical arbitrary floating-point output across
native and WASM. Input, runtime and driver fingerprints are checked before/after.
A separate single run with the **shipped** WASM artifact gives 10.649 s in the same
local API and matches those physical summaries; it is excluded from the medians.
The artifact is unchanged in the game. No external service/provider is called.

An exploratory first matrix made unnecessary full-result JSON conversions merely
for validation. Its RSS was contaminated; that whole matrix is excluded. The final
runner validates small summaries and measures existing response lengths instead.
The original inputs/results remain under `/tmp/pitgun-racing-attribution-2026-11-07`.

## What this establishes

- The JSON API is about **2.02x slower in WASM/Node than native** here. This includes
  computation, resampling, serialization and string bindings; it is not a pure
  arithmetic compiler ratio or a promise about Chrome/Safari.
- Near-1-GiB native typed peak means the memory problem cannot be blamed solely on
  JS, WASM bindings, React or Three.js. It does not prove a leak or a generic-runtime
  flaw. Native JSON parsing is a different host representation from JS, so its
  larger peak does not establish that a native deployment would use more memory.
- The pull stream transports about **217.2 MiB**, then completion emits another
  **66.3 MiB**: **283.4 MiB of JSON** before any browser Worker transport/archive.
  These are total bytes across responses, not simultaneous live allocations.
- A several-second UI wait or a 30-second signed career cycle must not be equated
  to pure Solver CPU. The actual signed/browser phases need separate attribution.

## Code paths to investigate

1. [Solver full-history vectors](../crates/pitgun-racing-solver/src/lib.rs#L1599)
   accumulate many sample arrays over every lap, for each competitor. Lap-wise
   stepping preserves physical state but does not bound this retained history.
2. [Simulator completion](../crates/pitgun-racing-simulator/src/lib.rs#L3006)
   retains resolved competitor results while materializing final player telemetry.
   Player resampling occurs during progression and again for the complete output.
   Reuse is not automatically equivalent because sample grids/offsets may differ.
3. [Typed collection](../crates/pitgun-racing-simulator/src/lib.rs#L2990) and the
   [local JSON facade](../crates/pitgun-racing-simulator/src/local_stream.rs#L83)
   clone terminal output. The terminal pull and completion both serialize it.
   Changing observable wire behavior requires a versioned compatibility plan.
4. The game additionally parses/transfers data, adapts channels, hashes archives,
   packs/compresses them and reconstructs replay graphs. Prior browser traces
   identify long tasks there; this offline experiment does not measure them.

The immediate optimization scope is Racing Solver/Simulator data retention,
then terminal output/host transport and game archiving. Preserve the published
canonical result, precision, identities, evidence and replay. Do not silently
reduce telemetry resolution, alter physics, discard required proof, or replace
full observations with incompatible summaries. A future internal capture policy
must keep the public Solver default and verify exact race-output parity.
The generic contract/runtime and the domain-neutral Engineer envelope need no
speculative rewrite from this evidence. Lot 2 should keep observations compact.

## Reproduction

From this worktree, with a sibling game checkout containing the public fixture:

```sh
python3 scripts/prepare-racing-performance.py ../signed-buffered-recovery/docs/qa/buffered-start-evidence/monza-input.json catalogs/racing/v1.8.0 /tmp/pitgun-perf-new
cargo build --offline --release -p pitgun-racing-simulator --example racing_performance_probe
cargo build --offline --release -p pitgun-racing-simulator --features wasm --target wasm32-unknown-unknown
```

Generate a web binding from the resulting WASM **into the temporary directory**,
using wasm-bindgen 0.2.106 or the game's pinned fallback generator; do not run the
sync script, which replaces game assets. Then, on macOS:

```sh
python3 scripts/racing-performance-trials.py /tmp/pitgun-perf-new/input.json target/release/examples/racing_performance_probe /tmp/pitgun-perf-new/wasm/pitgun_racing_simulator.js /tmp/pitgun-perf-new/wasm/pitgun_racing_simulator_bg.wasm /tmp/pitgun-perf-new/trials
```

Use the actual shared `CARGO_TARGET_DIR` paths if set. Choose fresh output paths;
drivers refuse to overwrite evidence. Compiler/build time is excluded. Processes
are fresh but OS caches/post activity are not isolated: three samples are a local
diagnostic, not a p95 population or cold browser qualification. Invalid-mode,
input and error responses fail; physical-result assertions guard all final trials.

Validation of this initial screen: native/rebuilt-WASM release builds, nine guarded
runs, shipped-artifact smoke, exact preparer input hash, syntax/format and strict
probe Clippy pass. This initial diagnostic changed no production library,
catalogue, WASM asset, UI or release flag; the signed follow-up retains a wrapper fix.

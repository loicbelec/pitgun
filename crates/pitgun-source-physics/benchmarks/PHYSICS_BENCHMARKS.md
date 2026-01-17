# Physics Benchmarks

See `/benchmarks/README.md` for the global index and curated snapshots.

## What we measure

- `physics_only`: core simulation time (telemetry generation only).
- `physics_to_batches`: simulation + events mapping + batching (no JSON).
- `physics_to_batches_json`: same as above + JSON serialization.
- `node_bench_simulate_batches.js`: WASM `simulate_batches` cost in Node (no network).

## How to run

```bash
cargo bench -p pitgun-source-physics
```

```bash
wasm-pack build --target nodejs crates/pitgun-source-physics-wasm
node crates/pitgun-source-physics-wasm/examples/node_bench_simulate_batches.js
```

## How to interpret

- `< 10ms` mean: multi-sim or dense sampling is feasible.
- `10-50ms` mean: good for single-sim and light sampling.
- `> 50ms` mean: consider reducing `hz`, trimming allocations, or adding binary output.

Notes:
- WASM numbers are approximations vs browser runtime.
- For browser targets, run inside a WebWorker to avoid UI contention.

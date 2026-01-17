# Benchmarks

## Philosophy

- `/benchmarks/` is the global index.
- `/benchmarks/results/` holds curated, versioned snapshots.
- `/crates/<crate>/benchmarks/` contains runbooks and interpretation docs only.

## Runbooks

- pitgun-source-physics: `crates/pitgun-source-physics/benchmarks/PHYSICS_BENCHMARKS.md`
- pitgun-core: `crates/pitgun-core/benchmarks/README.md`

## Common commands

```bash
cargo bench -p pitgun-source-physics
cargo bench -p pitgun-core
```

```bash
wasm-pack build --target nodejs crates/pitgun-source-physics-wasm
node crates/pitgun-source-physics-wasm/examples/node_bench_simulate_batches.js
```

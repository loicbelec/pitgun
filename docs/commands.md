# Supported Pitgun commands

This page contains commands that run against the current workspace. Historical
emulator, dataset, and prototype-manifest commands were removed because their
inputs or binaries no longer existed.

## Verified deterministic Racing demo

```bash
cargo run -p pitgun-cli -- demo racing --seed 42
```

Choose an exact Run Bundle destination when experimenting or scripting:

```bash
cargo run -p pitgun-cli -- demo racing --seed 42 --output /tmp/pitgun-racing-42
```

Verify that bundle in a fresh process without executing the simulator:

```bash
cargo run -p pitgun-cli -- replay /tmp/pitgun-racing-42
```

The bundle layout and collision rules are defined by
[Run Bundle V1](RUN_BUNDLE_V1.md).

## Observed-data aggregation

```bash
cargo run -p pitgun-core --example observed_segment_aggregation
```

This secondary example aggregates observed samples by a segment key. See
[Segment aggregation](segment_aggregation.md) for its semantics and its future
role in simulation-to-operation comparison.

## Workspace validation

```bash
cargo test --all
cargo bench -p pitgun-core --bench formula_processor_bench
```

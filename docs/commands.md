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

## Machine-readable Racing runs

Execute one fully resolved scenario and emit a compact canonical JSON result:

```bash
cargo run -p pitgun-cli -- run racing \
  --scenario apps/pitgun-cli/scenarios/racing-demo-v1.json \
  --seed 42
```

Write the compact result to a file and optionally retain the full immutable
evidence bundle:

```bash
cargo run -p pitgun-cli -- run racing \
  --scenario apps/pitgun-cli/scenarios/racing-demo-v1.json \
  --seed 42 \
  --result /tmp/pitgun-result.json \
  --bundle /tmp/pitgun-run-bundle
```

This command is the local automation boundary for parameter sweeps. It has no
network, database, or Databricks dependency. Its input, output, and structured
failure contracts are documented in
[Racing Batch Runner V1](RACING_BATCH_RUNNER_V1.md).

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

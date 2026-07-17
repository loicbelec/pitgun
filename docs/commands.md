# Pitgun command log

## Deterministic Racing demo

Run the built-in versioned scenario with an explicit deterministic seed:

```bash
cargo run -p pitgun-cli -- demo racing --seed 42
```

The command executes the scenario, collects typed telemetry, calculates the
observed maximum speed through a generic telemetry aggregator, and persists an
immutable bundle below `./pitgun-runs/`. It reports `SIMULATED`; replay and
`VERIFIED` are delivered by the remaining
[end-to-end demo work](https://github.com/loicbelec/pitgun/issues/49).

Choose an exact destination when experimenting or scripting:

```bash
cargo run -p pitgun-cli -- demo racing --seed 42 --output /tmp/pitgun-racing-42
```

The V1 files and collision rules are described in the
[run-bundle contract](RUN_BUNDLE_V1.md).

## Emulator
cargo run --bin pitgun-emulator -- \
  --target 127.0.0.1:5001 \
  --input nEngine=datasets/telemetry/FIA-nEngine.csv \
  --input rThrottleR=datasets/telemetry/Controller-rThrottleR.csv \
  --pace

## Segment aggregation demo (NLap + nEngine)
cargo run -p pitgun-emulator --release -- \
  --target 127.0.0.1:5001 \
  --input NLap=datasets/synthetic/NLap-demo.csv \
  --input nEngine=datasets/synthetic/nEngine-demo.csv \
  --pace

cargo run -p pitgun-cli -- subscribe \
  --bind 127.0.0.1:5001 \
  --config examples/manifests/pipeline/segment_aggregate_engine.yaml

## Python emulator receiver
python scripts/recv_pitgun.py       

## CLI receiver
cargo run --bin pitgun -- subscribe \
  --bind 127.0.0.1:5001 \
  --json

## Dummy Pitgun
cargo run -p pitgun-cli -- subscribe --config pitgun.yaml

## Benchmark
cargo bench -p pitgun-core --bench formula_processor_bench

# Suspension load
cargo run -p pitgun-emulator --release -- \
  --target 127.0.0.1:5001 \
  --input ChassisMaths-FPushRodFL=datasets/telemetry/ChassisMaths-FPushRodFL.csv \
  --input ChassisMaths-FPushRodFR=datasets/telemetry/ChassisMaths-FPushRodFR.csv \
  --input Chassis-FPushRodRR=datasets/telemetry/Chassis-FPushRodRR.csv \
  --input Chassis-FPushRodRL=datasets/telemetry/Chassis-FPushRodRL.csv \
  --pace


# Emulator (replays NLap and nEngine)
cargo run -p pitgun-emulator --release -- \
  --target 127.0.0.1:5001 \
  --input NLap=datasets/telemetry/Chassis-NLap.csv \
  --input nEngine=datasets/telemetry/FIA-nEngine.csv \
  --pace

# CLI
  cargo run -p pitgun-cli -- subscribe \
  --bind 127.0.0.1:5001 \
  --config examples/manifests/pipeline/segment_aggregate_engine.yaml

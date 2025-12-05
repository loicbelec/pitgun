# Pitgun command log

## Emulator
cargo run --bin pitgun-emulator -- \
  --target 127.0.0.1:5001 \
  --input nEngine=datasets/telemetry/FIA-nEngine.csv \
  --input rThrottleR=datasets/telemetry/Controller-rThrottleR.csv \
  --pace

## Python emulator receiver
python scripts/recv_pitgun.py       

## CLI receiver
cargo run --bin pitgun-cli -- subscribe \
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

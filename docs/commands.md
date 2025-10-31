# Pitgun command log

## Emulator
cargo run --bin pitgun-emulator -- \
  --target 127.0.0.1:5001 \
  --input nEngine=datasets/telemetry/RUN-001/FIA-nEngine.csv \
  --input throttle=datasets/telemetry/RUN-001/Controller-rThrottleR.csv \
  --pace

## Python receiver
python scripts/recv_pitgun.py       

## CLI receiver
cargo run --bin pitgun-cli -- subscribe \
  --bind 127.0.0.1:5001 \
  --json
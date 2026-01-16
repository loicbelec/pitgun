# Pitgun command log

## Emulator
```
cargo run -p pitgun-emulator -- \
  --target 127.0.0.1:5001 \
  --input nEngine=data/inputs/telemetry/nEngine.csv \
  --input throttle=data/inputs/telemetry/rThrottle.csv \
  --pace
```

## Game physics UDP emitter
```
cargo run -p pitgun-source-physics --bin physics_udp_emitter -- \
  --target 127.0.0.1:5001 \
  --request-json examples/sim_request.json \
  --pace
```

## Game physics HTTP emitter (telemetryd)
```
PITGUN_TELEMETRY_DATA_DIR=./data/ingest cargo run -p pitgun-telemetryd --release
```

```
PITGUN_CONTRACT_TTL_MS=600000 cargo run -p pitgun-configd --release
```

```
curl -sS -X POST http://127.0.0.1:8080/v1/requests/game \
  -H 'content-type: application/json' \
  -d @examples/sim_request.json \
  > tmp/sim_contract.json
```

```
cargo run -p pitgun-source-physics --bin physics_http_emitter -- \
  --telemetryd-url http://127.0.0.1:8080/beacon \
  --contract-json tmp/sim_contract.json \
  --verify-signature
```

WebSocket mode:
```
cargo run -p pitgun-source-physics --bin physics_http_emitter -- \
  --ws-url ws://127.0.0.1:8080/ws \
  --contract-json tmp/sim_contract.json
```

## CLI receiver
```
cargo run --bin pitgun-cli -- subscribe \
  --bind 127.0.0.1:5001 \
  --json
```

## Benchmark
```
cargo bench -p pitgun-core --bench formula_processor_bench
```

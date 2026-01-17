# pitgun-source-physics-wasm

Minimal WASM wrapper for the physics engine. Exposes a single function:

- `simulate_batches(request_json: &str) -> String`

It accepts a `GameSimulationRequestV1` JSON payload and returns a JSON array of
SessionEnvelope objects (compatible with `pitgun-telemetryd` /beacon and /ws).

## Build
```bash
wasm-pack build --target nodejs crates/pitgun-source-physics-wasm
```

## MVP usage
1) Build the WASM package:
```bash
wasm-pack build --target nodejs crates/pitgun-source-physics-wasm
```

2) Simulate batches and print basic stats:
```bash
node crates/pitgun-source-physics-wasm/examples/node_simulate_batches.js
```

3) Ingest over WebSocket (requires telemetryd + ws):
```bash
PITGUN_TELEMETRY_BIND=127.0.0.1:8080 \
PITGUN_TELEMETRY_DATA_DIR=/tmp/pitgun-telemetry-data \
cargo run -p pitgun-telemetryd --release
```

```bash
npm install --prefix crates/pitgun-source-physics-wasm
node crates/pitgun-source-physics-wasm/examples/node_ws_ingest_batches.js
```

Canonical local E2E workflow (native): see `justfile` in the repo root.

## Tests
```bash
wasm-pack test --node crates/pitgun-source-physics-wasm
```

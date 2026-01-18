# pitgun-source-physics-wasm

Minimal WASM wrapper for the physics engine. Exposes two functions:

- `simulate_batches(request_json: &str) -> String`
- `simulate_session_json(session_request_json: String) -> String`

`simulate_batches` accepts a `GameSimulationRequestV1` JSON payload and returns
a JSON array of SessionEnvelope objects (compatible with `pitgun-telemetryd` /beacon and /ws).

`simulate_session_json` accepts a `GameSessionRequestV1` JSON payload and returns
session standings, summaries, and player telemetry batches.

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

3) Simulate a session and print standings + telemetry stats:
```bash
node crates/pitgun-source-physics-wasm/examples/node_simulate_session.js
```

4) Session benchmark:
```bash
node crates/pitgun-source-physics-wasm/examples/node_bench_simulate_session.js
```

5) Weekend driver (FP1/FP2/FP3/RACE):
```bash
node crates/pitgun-source-physics-wasm/examples/node_weekend_driver.js --track demo-oval
```

## Session simulation example
- The player can adjust tuning between sessions (practice/race).
- Only player telemetry batches are returned.
- Competitors return summaries only (no telemetry).

Canonical local E2E workflow (native): see `justfile` in the repo root.

## Tests
```bash
wasm-pack test --node crates/pitgun-source-physics-wasm
```

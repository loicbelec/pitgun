# pitgun-source-physics-wasm

WASM wrapper around `pitgun-source-physics` that validates a signed simulation
contract and emits deterministic event batches.

## Build

```
wasm-pack build crates/pitgun-source-physics-wasm --target web
```

## Smoke test (Node)

```
PITGUN_SIGNING_SECRET=unit-test-secret \
  wasm-pack build --target nodejs crates/pitgun-source-physics-wasm
node crates/pitgun-source-physics-wasm/examples/node_smoke.js
```

## Environment

Environment variables are not accessible inside WASM modules. For Node/dev
verification, read `PITGUN_SIGNING_SECRET` in JS and pass it to
`PhysicsWasm.new_with_secret`. Browser usage should call `new()` without
signature verification.

## Minimal JS usage

```js
import init, { PhysicsWasm } from "./pkg/pitgun_source_physics_wasm.js";

await init();
const sim = new PhysicsWasm(signedContractJson);
const batch = sim.next_batch();
console.log(batch.end_of_stream, batch.events.length);
```

```js
import init, { PhysicsWasm } from "./pkg/pitgun_source_physics_wasm.js";

await init();
const sim = PhysicsWasm.new_with_secret(signedContractJson, signingSecret);
const batch = sim.next_batch();
console.log(batch.end_of_stream, batch.events.length);
```

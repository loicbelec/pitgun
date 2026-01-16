# pitgun-source-physics-wasm

WASM wrapper around the Pitgun physics engine. It accepts a
`GameSimulationRequestV1` JSON payload and returns a `GameSimulationResultV1`
JSON payload.

## Build

```
wasm-pack build crates/pitgun-source-physics-wasm --target web
```

## Minimal JS usage

```js
import init, { simulate } from "./pkg/pitgun_source_physics_wasm.js";

await init();

const request = {
  track_id: "demo-oval",
  hz: 60,
  tuning: {
    aero_points: 10,
    chassis_points: 10,
    engine_points: 10,
    cooling_points: 10,
    downforce_slider: 0.5,
    gear_ratio_slider: 0.5,
  },
  seed: 1,
  engine_version: "0.1.0",
};

const resultJson = simulate(JSON.stringify(request));
const result = JSON.parse(resultJson);
console.log(result.lap_time_s, result.summary.max_speed_kph);
```

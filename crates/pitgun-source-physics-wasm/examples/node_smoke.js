const wasm = require("../pkg/pitgun_source_physics_wasm.js");

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

const resultJson = wasm.simulate(JSON.stringify(request));
const result = JSON.parse(resultJson);

if (!result.telemetry || result.telemetry.length === 0) {
  throw new Error("expected telemetry array");
}

const first = result.telemetry[0];
if (typeof first.speed_kph !== "number" || typeof first.rpm !== "number") {
  throw new Error("expected speed_kph and rpm fields");
}

console.log("ok", {
  lap_time_s: result.lap_time_s,
  points: result.telemetry.length,
});

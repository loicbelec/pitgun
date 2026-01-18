const { performance } = require('node:perf_hooks');
const wasm = require('../pkg');

const request = {
  track_id: 'demo-oval',
  seed: 4242,
  hz: 10.0,
  laps: 5,
  player_tuning: {
    aero_points: 10,
    chassis_points: 10,
    engine_points: 10,
    cooling_points: 10,
    downforce_slider: 0.5,
    gear_ratio_slider: 0.5,
  },
  competitor_count: 9,
  total_budget_t: 40,
};

const warmupIterations = 10;
const iterations = 100;

let lastJson = null;

for (let i = 0; i < warmupIterations; i += 1) {
  lastJson = wasm.simulate_session_json(JSON.stringify(request));
}

const samples = [];
for (let i = 0; i < iterations; i += 1) {
  const start = performance.now();
  const resultJson = wasm.simulate_session_json(JSON.stringify(request));
  const end = performance.now();
  samples.push(end - start);
  lastJson = resultJson;
}

if (!lastJson) {
  throw new Error('simulate_session_json produced no output');
}

const result = JSON.parse(lastJson);
if (result.error) {
  throw new Error(result.error);
}

const playerBatches = result.player_batches || [];
const totalEvents = playerBatches.reduce(
  (sum, envelope) => sum + envelope.batch.events.length,
  0
);
const eosCount = playerBatches.filter((envelope) => envelope.batch.end_of_stream).length;

const sorted = [...samples].sort((a, b) => a - b);
const mean = samples.reduce((sum, value) => sum + value, 0) / samples.length;
const p50 = sorted[Math.floor(0.5 * (sorted.length - 1))];
const p95 = sorted[Math.floor(0.95 * (sorted.length - 1))];

console.log(`iterations=${iterations}`);
console.log(`mean_ms=${mean.toFixed(3)}`);
console.log(`p50_ms=${p50.toFixed(3)}`);
console.log(`p95_ms=${p95.toFixed(3)}`);
console.log(`batches=${playerBatches.length}`);
console.log(`events=${totalEvents}`);
console.log(`end_of_stream_count=${eosCount}`);

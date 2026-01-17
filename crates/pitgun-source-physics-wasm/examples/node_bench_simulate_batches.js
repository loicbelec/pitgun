const fs = require('fs');
const path = require('path');
const { performance } = require('node:perf_hooks');

const wasm = require('../pkg');

const requestPath = path.join(
  __dirname,
  '..',
  '..',
  'pitgun-source-physics',
  'fixtures',
  'requests',
  'demo-oval_default.json'
);
const requestJson = fs.readFileSync(requestPath, 'utf8');

const warmupIterations = 10;
const iterations = 100;

let lastJson = null;
let lastLen = 0;

for (let i = 0; i < warmupIterations; i += 1) {
  lastJson = wasm.simulate_batches(requestJson);
  lastLen = lastJson.length;
}

const samples = [];
for (let i = 0; i < iterations; i += 1) {
  const start = performance.now();
  const batchesJson = wasm.simulate_batches(requestJson);
  const end = performance.now();
  samples.push(end - start);
  lastJson = batchesJson;
  lastLen = batchesJson.length;
}

if (!lastJson) {
  throw new Error('simulate_batches produced no output');
}

const batches = JSON.parse(lastJson);
const totalEvents = batches.reduce(
  (sum, envelope) => sum + envelope.batch.events.length,
  0
);
const endOfStreamCount = batches.filter(
  (envelope) => envelope.batch.end_of_stream
).length;

const sorted = [...samples].sort((a, b) => a - b);
const mean = samples.reduce((sum, value) => sum + value, 0) / samples.length;
const p50 = sorted[Math.floor(0.50 * (sorted.length - 1))];
const p95 = sorted[Math.floor(0.95 * (sorted.length - 1))];

console.log(`iterations=${iterations}`);
console.log(`mean_ms=${mean.toFixed(3)}`);
console.log(`p50_ms=${p50.toFixed(3)}`);
console.log(`p95_ms=${p95.toFixed(3)}`);
console.log(`batches=${batches.length}`);
console.log(`events=${totalEvents}`);
console.log(`end_of_stream_count=${endOfStreamCount}`);
console.log(`last_json_len=${lastLen}`);

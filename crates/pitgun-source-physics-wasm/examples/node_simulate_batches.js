const fs = require('fs');
const path = require('path');

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

const batchesJson = wasm.simulate_batches(requestJson);
const batches = JSON.parse(batchesJson);

if (!Array.isArray(batches) || batches.length === 0) {
  throw new Error('simulate_batches returned no batches');
}

const firstBatchEvents = batches[0].batch.events.length;
const lastEndOfStream = batches[batches.length - 1].batch.end_of_stream;

console.log(`batches=${batches.length}`);
console.log(`first_batch_events=${firstBatchEvents}`);
console.log(`last_end_of_stream=${lastEndOfStream}`);

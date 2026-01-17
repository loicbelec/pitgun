const fs = require('fs');
const path = require('path');
// Requires: npm install ws
const WebSocket = require('ws');

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

const ws = new WebSocket('ws://127.0.0.1:8080/ws');

ws.on('open', () => {
  for (const envelope of batches) {
    ws.send(JSON.stringify(envelope));
  }
  ws.close();
});

ws.on('error', (err) => {
  console.error('ws error:', err);
  process.exit(1);
});

const WebSocket = require('ws');
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

const resultJson = wasm.simulate_session_json(JSON.stringify(request));
const result = JSON.parse(resultJson);

if (result.error) {
  throw new Error(result.error);
}

const playerBatches = result.player_batches || [];

const ws = new WebSocket('ws://127.0.0.1:8080/ws');

ws.on('open', () => {
  for (const envelope of playerBatches) {
    ws.send(JSON.stringify(envelope));
  }
  ws.close();
  console.log(`sent_batches=${playerBatches.length}`);
});

ws.on('error', (err) => {
  console.error('ws error:', err);
  process.exit(1);
});

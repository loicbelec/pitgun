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

const standings = result.standings || [];
const playerSummary = result.player_summary;
const playerIndex = standings.findIndex((entry) => entry.kind === 'player');
const playerPosition = playerIndex >= 0 ? playerIndex + 1 : 'n/a';

const p1Time = standings.length > 0 ? standings[0].total_time_s : 0;
const playerTime = playerSummary ? playerSummary.total_time_s : 0;
const gap = playerTime - p1Time;

const playerBatches = result.player_batches || [];
const totalEvents = playerBatches.reduce(
  (sum, envelope) => sum + envelope.batch.events.length,
  0
);

console.log('--- Session Result ---');
console.log(`player_position=${playerPosition}/${standings.length}`);
console.log(`p1_total_time_s=${p1Time.toFixed(3)}`);
console.log(`player_total_time_s=${playerTime.toFixed(3)}`);
console.log(`gap_to_p1_s=${gap.toFixed(3)}`);
console.log(`player_avg_lap_s=${playerSummary.avg_lap_s.toFixed(3)}`);
console.log(`competitors=${result.competitor_summaries.length}`);
console.log(`player_batches=${playerBatches.length}`);
console.log(`player_events=${totalEvents}`);

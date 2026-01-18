const wasm = require('../pkg');

const request = {
  track_id: 'demo-oval',
  seed: 2026,
  hz: 10.0,
  laps: 10,
  player_tuning: {
    aero_points: 12,
    chassis_points: 8,
    engine_points: 14,
    cooling_points: 6,
    downforce_slider: 0.6,
    gear_ratio_slider: 0.4,
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
const competitorSummaries = new Map(
  (result.competitor_summaries || []).map((summary) => [
    summary.competitor_id,
    summary,
  ])
);

const leaderTime = standings.length > 0 ? standings[0].total_time_s : 0;

const avgLapFor = (entry) => {
  if (entry.kind === 'player') {
    return playerSummary.avg_lap_s;
  }
  const summary = competitorSummaries.get(entry.competitor_id);
  return summary ? summary.avg_lap_s : null;
};

console.log('--- Race Session ---');
standings.forEach((entry, index) => {
  const avgLap = avgLapFor(entry);
  const gap = entry.total_time_s - leaderTime;
  const gapLabel = index === 0 ? 'leader' : `+${gap.toFixed(3)}s`;
  const avgLabel = avgLap == null ? 'n/a' : `${avgLap.toFixed(3)}s`;
  console.log(
    `${index + 1}. ${entry.display_name} total=${entry.total_time_s.toFixed(3)}s avg=${avgLabel} ${gapLabel}`
  );
});

const playerIndex = standings.findIndex((entry) => entry.kind === 'player');
const playerPosition = playerIndex >= 0 ? playerIndex + 1 : 'n/a';

const playerBatches = result.player_batches || [];
const playerEvents = playerBatches.reduce(
  (sum, envelope) => sum + envelope.batch.events.length,
  0
);

console.log(`player_position=${playerPosition}/${standings.length}`);
console.log(`competitors=${result.competitor_summaries.length}`);
console.log(`player_batches=${playerBatches.length}`);
console.log(`player_events=${playerEvents}`);

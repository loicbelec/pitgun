const WebSocket = require('ws');
const wasm = require('../pkg');

const DEFAULT_TUNING = {
  aero_points: 10,
  chassis_points: 10,
  engine_points: 10,
  cooling_points: 10,
  downforce_slider: 0.5,
  gear_ratio_slider: 0.5,
};

const clamp = (value, min, max) => Math.min(Math.max(value, min), max);

const parseArgs = () => {
  const args = process.argv.slice(2);
  const config = {
    track: 'demo-oval',
    fpLaps: 5,
    raceLaps: 10,
    hz: 10.0,
    weekendSeed: 1,
    ingestWs: null,
  };

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--track') {
      config.track = args[i + 1];
      i += 1;
    } else if (arg === '--fp-laps') {
      config.fpLaps = Number(args[i + 1]);
      i += 1;
    } else if (arg === '--race-laps') {
      config.raceLaps = Number(args[i + 1]);
      i += 1;
    } else if (arg === '--hz') {
      config.hz = Number(args[i + 1]);
      i += 1;
    } else if (arg === '--weekend-seed') {
      config.weekendSeed = Number(args[i + 1]);
      i += 1;
    } else if (arg === '--ingest-ws') {
      config.ingestWs = args[i + 1];
      i += 1;
    }
  }

  return config;
};

const applyTuningTweaks = (tuning, step) => {
  const next = { ...tuning };
  if (step === 1) {
    next.aero_points += 1;
    next.engine_points -= 1;
    next.downforce_slider += 0.03;
    next.gear_ratio_slider -= 0.02;
  } else if (step === 2) {
    next.chassis_points += 1;
    next.cooling_points -= 1;
    next.downforce_slider -= 0.02;
    next.gear_ratio_slider += 0.04;
  } else if (step === 3) {
    next.aero_points -= 1;
    next.engine_points += 1;
    next.downforce_slider += 0.01;
    next.gear_ratio_slider -= 0.01;
  }

  next.aero_points = clamp(next.aero_points, 0, 20);
  next.chassis_points = clamp(next.chassis_points, 0, 20);
  next.engine_points = clamp(next.engine_points, 0, 20);
  next.cooling_points = clamp(next.cooling_points, 0, 20);
  next.downforce_slider = clamp(next.downforce_slider, 0.0, 1.0);
  next.gear_ratio_slider = clamp(next.gear_ratio_slider, 0.0, 1.0);

  const total =
    next.aero_points +
    next.chassis_points +
    next.engine_points +
    next.cooling_points;
  const target =
    tuning.aero_points +
    tuning.chassis_points +
    tuning.engine_points +
    tuning.cooling_points;
  const diff = target - total;

  if (diff !== 0) {
    const adjusted = clamp(next.engine_points + diff, 0, 20);
    next.engine_points = adjusted;
  }

  return next;
};

const formatTuning = (tuning) =>
  `aero=${tuning.aero_points} chassis=${tuning.chassis_points} engine=${tuning.engine_points} cooling=${tuning.cooling_points} downforce=${tuning.downforce_slider.toFixed(2)} gear=${tuning.gear_ratio_slider.toFixed(2)}`;

const buildRequest = (config, tuning, laps) => ({
  track_id: config.track,
  seed: config.weekendSeed,
  hz: config.hz,
  laps,
  player_tuning: tuning,
  competitor_count: 9,
  total_budget_t:
    tuning.aero_points +
    tuning.chassis_points +
    tuning.engine_points +
    tuning.cooling_points,
});

const summarizeSession = (label, result) => {
  const standings = result.standings || [];
  const playerSummary = result.player_summary;
  const playerIndex = standings.findIndex((entry) => entry.kind === 'player');
  const playerPosition = playerIndex >= 0 ? playerIndex + 1 : 'n/a';
  const p1Time = standings.length > 0 ? standings[0].total_time_s : 0;
  const playerTime = playerSummary ? playerSummary.total_time_s : 0;
  const gap = playerTime - p1Time;
  const playerBatches = result.player_batches || [];
  const playerEvents = playerBatches.reduce(
    (sum, envelope) => sum + envelope.batch.events.length,
    0
  );

  console.log(`\n--- ${label} ---`);
  console.log(`player_position=${playerPosition}/10`);
  console.log(`p1_total_time_s=${p1Time.toFixed(3)}`);
  console.log(`player_total_time_s=${playerTime.toFixed(3)}`);
  console.log(`gap_to_p1_s=${gap.toFixed(3)}`);
  console.log(`player_avg_lap_s=${playerSummary.avg_lap_s.toFixed(3)}`);
  console.log(`player_batches=${playerBatches.length}`);
  console.log(`player_events=${playerEvents}`);

  return {
    session_type: label,
    player_position: playerPosition,
    p1_total_time_s: p1Time,
    player_total_time_s: playerTime,
    gap_to_p1_s: gap,
    player_avg_lap_s: playerSummary.avg_lap_s,
    player_batches: playerBatches.length,
    player_events: playerEvents,
  };
};

const runSession = (config, label, tuning, laps) => {
  console.log(`\n${label} tuning: ${formatTuning(tuning)}`);
  const request = buildRequest(config, tuning, laps);
  const resultJson = wasm.simulate_session_json(JSON.stringify(request));
  const result = JSON.parse(resultJson);
  if (result.error) {
    if (result.error.includes('unknown track_id')) {
      const tracksJson = wasm.list_tracks_json();
      const tracks = JSON.parse(tracksJson);
      console.error(`unknown track_id '${config.track}'`);
      console.error(`available_tracks=${tracks.join(',')}`);
    }
    throw new Error(result.error);
  }
  return { result, summary: summarizeSession(label, result) };
};

const main = async () => {
  const config = parseArgs();
  const sessions = [
    { label: 'FP1', laps: config.fpLaps, tweakStep: 1 },
    { label: 'FP2', laps: config.fpLaps, tweakStep: 2 },
    { label: 'FP3', laps: config.fpLaps, tweakStep: 3 },
    { label: 'RACE', laps: config.raceLaps, tweakStep: null },
  ];

  let tuning = { ...DEFAULT_TUNING };
  const weekendSummary = [];
  let raceResult = null;

  for (const session of sessions) {
    const { result, summary } = runSession(
      config,
      session.label,
      tuning,
      session.laps
    );
    weekendSummary.push(summary);

    if (session.label === 'RACE') {
      raceResult = result;
    }

    if (session.tweakStep) {
      tuning = applyTuningTweaks(tuning, session.tweakStep);
    }
  }

  console.log('\n=== Weekend Summary ===');
  weekendSummary.forEach((entry) => {
    console.log(
      `${entry.session_type}: pos=${entry.player_position} p1=${entry.p1_total_time_s.toFixed(3)}s player=${entry.player_total_time_s.toFixed(3)}s gap=${entry.gap_to_p1_s.toFixed(3)}s avg=${entry.player_avg_lap_s.toFixed(3)}s batches=${entry.player_batches}`
    );
  });

  if (config.ingestWs && raceResult) {
    const playerBatches = raceResult.player_batches || [];
    if (playerBatches.length === 0) {
      console.log('no_batches_to_send');
      return;
    }

    const ws = new WebSocket(config.ingestWs);
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
  }
};

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

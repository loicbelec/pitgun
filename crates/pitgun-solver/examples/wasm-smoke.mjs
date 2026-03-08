import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  initSync,
  catalog_json,
  run_simulation_json,
} from "../pkg/pitgun_solver.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const wasmPath = path.resolve(here, "../pkg/pitgun_solver_bg.wasm");
const wasmBytes = fs.readFileSync(wasmPath);

initSync({ module: wasmBytes });

const catalog = JSON.parse(catalog_json());
console.log("circuits", catalog.circuits.slice(0, 5).map((entry) => entry.id));

const request = {
  input: {
    track_id: "it-1922",
    laps: 1,
    competitors: [
      {
        id: "player",
        driver_id: "default",
        name: "Player",
        team_id: "team",
        is_player: true,
        tuning: {
          engine_points: 25,
          cooling_points: 25,
          aero_points: 25,
          chassis_points: 25,
          downforce_slider: 0.5,
          gear_ratio_slider: 0.5,
        },
        budget_cap: 100,
      },
    ],
    vehicle_id: "f1_2026",
    era: 2026,
    hz: 20,
  },
  seed: 7,
  era: 2026,
  hz: 20,
};

const result = JSON.parse(run_simulation_json(JSON.stringify(request)));
if (result.error) {
  throw new Error(result.error);
}

const frames = result.player_batches.flatMap((batch) => batch.frames);
console.log("standings", result.standings);
console.log("player_telemetry_frames", frames.length);
console.log("first_frame_sampling_hz", frames[0]?.metadata?.sampling_hz);

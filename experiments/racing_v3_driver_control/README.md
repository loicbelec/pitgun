# Racing Model V3 driver-control screening

This offline campaign screens candidate Model `0.12.0`. It does not change the
game, Racing Catalog, Authority, Verifier, WASM package, staging, or production.

The campaign keeps the vehicle, component selection, 150 kg screening fuel reserve,
development budget and setup constant. Only the versioned driver traits, explicit driving mode,
circuit, tire, seed and session horizon vary. The Rust Solver remains the sole
physical implementation; Python generates scenarios and analyzes its compact
diagnostics.

## Matrix

The complete local screen contains 702 deterministic executions:

- 648 full-factorial runs across four trade-off archetypes, three modes,
  Monaco/Suzuka/Monza, soft/medium/hard, three seeds, and short/race-length
  horizons;
- 36 controlled consistency comparisons where the other traits are fixed;
- 18 controlled tire-management comparisons where the other traits are fixed.

Race-length means the real GP lap count used by the game for each selected
circuit. The screen therefore executes 23,790 simulated laps rather than
extrapolating a short-run result.

The 150 kg common reserve is not a proposed game or catalog fuel target. Pilot
runs depleted 100 kg at Monaco and 110 kg at Suzuka before the end of the real
GP distance. The deliberately conservative reserve prevents fuel exhaustion
from censoring driver/tire comparisons; every member of a pair carries the
same mass.

The profile and roster are calibration candidates, not accepted catalog
resources. Their content digests enter every experimental natural key.

## Run locally

```bash
cargo build --locked --release \
  -p pitgun-racing-simulator --example v3_driver_control_probe
python3 experiments/racing_v3_driver_control/screen_local.py --jobs 4
python3 experiments/racing_v3_driver_control/screen_local.py --jobs 4 --check
```

For probe development only, `--limit N` executes the first `N` configurations
and writes a deliberately incomplete report.

The canonical report is
[`results/local-driver-control-screen-v1.json`](results/local-driver-control-screen-v1.json).
It records content identities, pace and dispersion, resolved utilization,
control error, correction workload, correction heat and correction-attributed
wear. Its review verdict is evidence for refinement or Databricks replay; it
never promotes a model automatically.

## Local finding

The seed coefficient set receives a `REFINE` verdict. Every physical direction
is active: attack is quicker and pays more error/workload, manage preserves
tires, consistency changes dispersion, and tire management reduces
correction-attributed wear. However, `attack` remains the fastest mode in all
54 reviewed scenario groups. The mechanics are connected, but their current
cost is too small to make mode selection a strategic decision. Databricks must
therefore explore the coefficient surface rather than merely replaying this
seed unchanged.

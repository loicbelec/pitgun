# Racing Model V3 driver-control screening

The preserved V1 report screened candidate Model `0.12.0`. The current V2
screen evaluates candidate `0.13.0`. Neither changes the
game, Racing Catalog, Authority, Verifier, WASM package, staging, or production.

Candidate `0.13.0` preserves every reviewed `0.12.0` coefficient and adds the
smallest structural correction revealed by the Databricks V2 campaign:
correction workload now reserves part of the same aggregate tire-force budget
used for useful braking, cornering and traction. The reservation follows
`1 / sqrt(correction_workload_multiplier)`, so it is derived from existing
workload lineage rather than introduced as a hidden lap-time penalty.

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
python3 experiments/racing_v3_driver_control/screen_v11_parameter_surface.py --jobs 8
python3 experiments/racing_v3_driver_control/screen_v11_parameter_surface.py --jobs 8 --check
python3 experiments/racing_v3_driver_control/review_v11_shortlist.py --check
```

For probe development only, `--limit N` executes the first `N` configurations
and writes a deliberately incomplete report.

The current canonical report is
[`results/local-driver-control-screen-v2.json`](results/local-driver-control-screen-v2.json).
The immutable
[`results/local-driver-control-screen-v1.json`](results/local-driver-control-screen-v1.json)
remains the `0.12.0` baseline.
It records content identities, pace and dispersion, resolved utilization,
control error, correction workload, correction heat and correction-attributed
wear. Its review verdict is evidence for refinement or Databricks replay; it
never promotes a model automatically.

The second reproducible report,
[`results/local-driver-friction-parameter-screen-v1.json`](results/local-driver-friction-parameter-screen-v1.json),
replays the exact 33-profile, 1,584-execution Databricks V2 surface while
changing only the offline experiment schema from V10 to V11. It therefore
separates the structural effect of the friction budget from coefficient
selection.

## Preserved 0.12.0 finding

The seed coefficient set receives a `REFINE` verdict. Every physical direction
is active: attack is quicker and pays more error/workload, manage preserves
tires, consistency changes dispersion, and tire management reduces
correction-attributed wear. However, `attack` remains the fastest mode in all
54 reviewed scenario groups. The mechanics are connected, but their current
cost is too small to make mode selection a strategic decision. Databricks must
therefore explore the coefficient surface rather than merely replaying this
seed unchanged.

## Preliminary 0.13.0 finding

The unchanged seed coefficients still receive `REFINE`: attack remains fastest
in all 54 groups of the independent 702-execution screen. The structural change
is nevertheless material. On the governed 33-profile exploratory surface, 20
profiles now pass the preliminary selection gate, compared with zero under
candidate 0.12. All 1,584 runs are finite and physically ordered; several
profiles make attack fastest in only four of eight short groups and four of
eight race-length groups.

This is evidence of a usable decision surface, not a selected AI policy. A
shortlist must next pass the reserved 702-execution matrix without coefficient
retuning before any profile can enter a catalog or the game.

## Independent shortlist holdout

The governed [`shortlist/shortlist-v1.json`](shortlist/shortlist-v1.json)
freezes three deliberately different profiles from the exploratory surface:

- `halton-11` applies a strong, near-linear correction cost;
- `halton-19` concentrates more of that cost near maximum commitment;
- `halton-27` tests a gentler policy with narrower mode commitments.

Each exact V11 profile is replayed over the independent 702-case matrix without
coefficient retuning: 2,106 deterministic Rust executions and 71,370 simulated
laps in total. All three profiles preserve the reviewed causal directions and
produce zero pathological executions. They also make `balanced` or `manage`
optimal for the `limit_specialist` in some contexts.

The holdout nevertheless rejects all three profiles. Across the 54 global
contexts of each profile, `smooth_operator:attack` is always the fastest
driver/mode combination. The reproducible review therefore records
`STRUCTURAL_REFINEMENT_REQUIRED`, selects no profile, and performs no catalog or
game publication. This is a useful negative result: the friction-budget change
creates meaningful mode trade-offs, while the remaining weakness is now
localized to the relative balance between driver archetypes.

The compact decision artifact is
[`results/holdout-driver-control-shortlist-v1.json`](results/holdout-driver-control-shortlist-v1.json).
It references the three complete per-profile reports and their content hashes.

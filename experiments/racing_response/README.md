# Racing response-surface experiment

This local experiment measures how the canonical Racing Solver responds to its
two existing setup sliders before any physical coefficient is recalibrated. It
uses the same resolved scenario and `pitgun run racing` boundary as Databricks;
Python only materializes the bounded grid and analyzes the versioned diagnostic
results.

The reviewed V1 protocol uses:

- five physical circuits: Monza, Monaco, Budapest, Suzuka, and Singapore;
- eleven equally spaced downforce levels from `0.0` to `1.0`;
- eleven equally spaced gearing levels from `0.0` to `1.0`;
- seed `42` and one lap per point;
- 605 canonical simulations in total.

Build the release runner and execute the experiment from the repository root:

```bash
cargo build --release -p pitgun-cli
python3 experiments/racing_response/response_surface.py --jobs 4
```

The deterministic report is written to
`experiments/racing_response/results/racing-response-surface-v1.json`. Parallel
job count affects wall-clock time only: points are sorted before canonical JSON
serialization. Its adjacent `.sha256` file identifies the exact report. Re-run
with `--check` to prove that both stored artifacts match the same runner and
inputs byte for byte.

The per-circuit summary reports the fastest point, whether it sits on a search
boundary, total lap-time range, and two isolated comparisons:

- maximum minus minimum downforce at midpoint gearing;
- longest minus shortest gearing at midpoint downforce.

Negative time deltas mean the first named condition is faster. The full grid is
retained with observed maximum speed and the complete setup-response diagnostic
for later visualization and governed campaign design. This experiment does not
modify catalog resources, model coefficients, game policy, or Databricks
tables.

The reviewed interpretation and next decision are documented in
[`RACING_RESPONSE_SURFACE_V1.md`](../../docs/RACING_RESPONSE_SURFACE_V1.md).

## Coefficient screening V1

After making the historical tuning response explicit, the second experiment
uses a Rust-only probe to screen bounded aerodynamic coefficient families. The
probe is deliberately absent from the game, WASM exports, catalog resources,
and production contracts.

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/coefficient_screening.py --jobs 4
```

The campaign crosses 25 downforce/drag response families with three circuits
and a five-by-five setup grid: 1,875 deterministic simulations. The full result
is stored in `results/racing-coefficient-screening-v1.json`. Its interpretation
and the reason no coefficient is published yet are documented in
[`RACING_COEFFICIENT_SCREENING_V1.md`](../../docs/RACING_COEFFICIENT_SCREENING_V1.md).

## Aerodynamic response refinement V1

The third local experiment refines the shortlisted aerodynamic neighborhood on
an eleven-by-eleven setup grid. It evaluates 42 candidate shapes plus the
historical response across Monza, Monaco, and Suzuka: 15,609 deterministic
simulations.

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/refine_aero_response.py --jobs 4
```

The report retains compact time and maximum-speed matrices plus a digest of all
execution points. The selected response-shape anchor, its remaining pace gap,
and the next base-coefficient calibration decision are documented in
[`RACING_AERO_RESPONSE_REFINEMENT_V1.md`](../../docs/RACING_AERO_RESPONSE_REFINEMENT_V1.md).

## Aerodynamic base calibration V1

The fourth local experiment fixes the selected response gains, derives the
neutral-preserving bases, and performs coarse then refined searches over
`drag_base` and `downforce_base`. Each stage executes 9,801 deterministic
points and stores a compact report; the refinement is linked to the coarse
calibration by digest.

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/calibrate_aero_bases.py --jobs 4
python3 experiments/racing_response/refine_aero_bases.py --jobs 4
```

The selected offline candidate, its reduced gameplay-pace gap, and the planned
governed Databricks replay are documented in
[`RACING_AERO_BASE_CALIBRATION_V1.md`](../../docs/RACING_AERO_BASE_CALIBRATION_V1.md).

## Mixed-circuit diagnosis V1

After the governed Databricks review exposed a Suzuka setup-label mismatch,
the fifth local experiment tests whether the calibrated response or the coarse
review grid is responsible. It evaluates 35 narrowly bounded response variants
on an eleven-by-eleven setup grid across five calibration circuits, with
Silverstone and Spa retained as post-selection holdouts: 29,645 deterministic
probe executions.

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/diagnose_mixed_circuit.py --jobs 4
```

The diagnosis keeps the current mixed-circuit response, identifies review-grid
aliasing at Suzuka, and records a separate high-speed guardrail failure found
at Spa. The evidence and decision boundary are documented in
[`RACING_MIXED_CIRCUIT_DIAGNOSIS_V1.md`](../../docs/RACING_MIXED_CIRCUIT_DIAGNOSIS_V1.md).

## Spa high-speed diagnosis V1

The sixth local experiment investigates the Spa holdout without changing the
calibrated response. A dedicated track-aware probe relates speed telemetry to
distance and curvature, then compares minimum and maximum downforce at eleven
gearing levels across Suzuka, Silverstone, and Spa: 66 deterministic runs.

```bash
cargo build --release -p pitgun-racing-simulator --example high_speed_response_probe
python3 experiments/racing_response/diagnose_spa_high_speed.py --jobs 4
```

The result reproduces Spa's `406.393 km/h` peak and eleven aggregate
corner-response failures, while showing that downforce still increases speed
in every high-curvature comparison. It identifies a general binary Solver
segmentation weakness exposed by Spa, records a failing high-speed holdout
guardrail, and preserves all five calibration optima. The evidence and the
reason no coefficient or circuit is changed automatically are documented in
[`RACING_SPA_HIGH_SPEED_DIAGNOSIS_V1.md`](../../docs/RACING_SPA_HIGH_SPEED_DIAGNOSIS_V1.md).

## Spa relief impact V1

The seventh local experiment isolates the validated SPW elevation profile from
every other model input. Both flat and relief variants use identical `s/x/y`,
vehicle, tuning response, seed, and five-by-five setup grid. Only `z` changes;
the Simulator derives slope through its existing `track_profile` boundary.

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/measure_spa_relief_impact.py --jobs 4
```

The 50 deterministic runs measure lap time, maximum speed, setup response, and
optimum movement without editing the catalog. Results and the non-promotion
decision are documented in
[`RACING_SPA_RELIEF_IMPACT_V1.md`](../../docs/RACING_SPA_RELIEF_IMPACT_V1.md).

## Spa relief-response localization V1

The eighth local experiment records the canonical Simulator's telemetry for a
flat Spa control and three SPW relief smoothing windows. It aligns speed,
brake, and acceleration on a 25-metre grid so the aggregate lap-time response
can be located and checked for smoothing sensitivity or vertical
discontinuities.

```bash
cargo build --release -p pitgun-racing-simulator --example relief_response_probe
python3 experiments/racing_response/localize_spa_relief_response.py
```

The committed evidence shows a 200–203 ms relief penalty across all three
windows, a 3 ms sensitivity spread, and less than 0.08 g of vertical response.
It remains experimental and does not promote a circuit resource. The method,
interpretation, and next boundary are documented in
[`RACING_SPA_RELIEF_LOCALIZATION_V1.md`](../../docs/RACING_SPA_RELIEF_LOCALIZATION_V1.md).

## Continuous curvature-response candidate V1

The ninth local experiment introduces a continuous aerodynamic response behind
an explicit Rust-only model boundary. Production, WASM, catalog, and hosted
verification remain on the legacy model while the candidate is replayed over
the governed eleven-by-eleven setup grid for seven circuits.

```bash
cargo build --release -p pitgun-racing-simulator \
  --example continuous_curvature_response_probe
python3 experiments/racing_response/evaluate_continuous_curvature_response.py \
  --jobs 4
```

The 847-run comparison brings the global maximum speed below 400 km/h but
retains one obsolete aggregate Spa invariant failure family and moves three
optima by one grid step. The candidate is therefore not promoted. Its isolation,
evidence, and next review boundary are documented in
[`RACING_CONTINUOUS_CURVATURE_RESPONSE_V1.md`](../../docs/RACING_CONTINUOUS_CURVATURE_RESPONSE_V1.md).

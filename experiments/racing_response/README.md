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

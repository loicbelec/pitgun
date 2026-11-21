# Cooling and tire gameplay audit V1

This audit tests the **existing** career-V9 laws and the optional energy candidate
before any Worker/WASM work. It changes no equation, coefficient, catalog, game
save, hosted service or runtime admission. Point/slider/resource resolution stays
in Rust. The previous energy slice did not introduce a new tire law.

## Questions and fixed evidence

The82 preregistered synthetic cases compare the historical and energy kernels:

- Equal-budget cooling response lines (8/15/20/40 effective envelopes, axis cap20),
  engine or chassis as the stated donor, three engine families, several circuits.
- Additional cooling at fixed engine/other points, explicitly **not equal budget**.
- Full Monza53 attempts with the unchanged fuel supply; no hidden refilling or
  larger reservoir to force successful comparisons.
- One reserved seed/circuit subset, a labeled +1 cooling bonus and static45°C air
  diagnostic. These do not reconstruct an earned HQ save or add a weather model.
-20-lap tire/compound/driver and stop-window runs; point/slider response cases.
- Controlled non-playable ablations: derating off; tire temperature feedback off
  (grip and wear); wear-to-grip feedback off; alternative initial tire temperature.

A finite response line is not an exhaustive setup optimization or proof that all
other settings must be positive. Zero **allocated** cooling differs from zero
**effective** cooling after bonuses, and neither means no physical cooling system.
Game preparation starts at8 points plus development bonuses and adds setup offsets;
the exact game files were inspected, but these native scenarios are synthetic
legal effective allocations, not a full progression/availability validation.

Before the campaign, the read-only stepper/diagnostic probe must reproduce all22
single-car historical/energy outcomes (including the same fuel failure) from the
frozen energy baseline. Complete native result digests remain the oracle.
The audit's success means reproducible evidence, **not** gameplay acceptance.
Failures, zero-cooling dominance, cold tires and grip-floor occupancy are results
to retain; do not change coefficients until this same campaign looks green.

## Run

From clean committed sources with cached dependencies:

```sh
cargo build --offline --locked --release -p pitgun-racing-simulator \
  --example thermal_tire_gameplay_probe
PYTHONDONTWRITEBYTECODE=1 python3 experiments/thermal_tire_gameplay/make_plan.py --check
PYTHONDONTWRITEBYTECODE=1 python3 experiments/thermal_tire_gameplay/run_local.py \
  --probe target/release/examples/thermal_tire_gameplay_probe
# Repeat with --check; no archived outcome is overwritten.
```

`make_plan.py` generates the explicit JSON once. Never rewrite a published V1 plan
or historical report after coefficient selection; use a new experiment identity.
Reports pin exact source/binary/plan/catalog hashes. The runner rejects stale
compiled sources and uses owned fresh Unix children with180s timeouts. It records
CPU/wall/RSS only as operational measurements, not a performance acceptance study.
No Databricks or inference/provider calls are made.

## Meaning of the metrics

Per-lap native arrays are captured before the next continuation; summaries use
interval-duration weights and left endpoint states. The new energy mode shares
its integration clock; the historical trajectory/thermal-clock approximation is
retained. These are native model traces, not an instrumented real vehicle.

Tire temperature, heating/cooling intervals, wear and the actual native grip law
are observed before pit service. The separately recorded `after_service` state
shows resets and must not be mistaken for tire heating during driving. The
`temperature_factor_at_least_95pct_time_fraction` threshold means at least95% of
peak **temperature factor**, not95% remaining life or a validated temperature
window. Grip-floor occupancy measures the current minimum multiplier masking
further cold/hot departures; it does not establish experimental tire safety.
The thermal/wear ablations establish model causality, not physical validity.

One aggregate contact patch/temperature is still used; the contact workload is an
empirical proxy rather than measured slip dissipation. Engine/ambient coefficients,
fixed four-pass tire coupling, no tire carcass/surface states or pavement model,
abstract pit reset, no engine damage and the power floor remain declared limits.
A tire may cool from a preheated start; monotonic mean cooling alone is not proof
of a bug. Persistent operation far from optimum plus a saturated grip floor and
weak setup/strategy consequences is a calibration/model-structure concern.

Future fixes should target measured mechanisms and have independently selected
validation cases; this audit does not fit new coefficients or promote gameplay.

## Follow-ups kept separate

The [results and interpretation](RESULTS_V1.md) retain the primary audit. The
subsequent `grid-plan-v1.json` preregisters187 allocations at fixed sliders;
`response_grid.py` runs both kernels and repeats the16 selected minima exactly.
`refinement-plan-v1.json` then preregisters36 finer cooling allocations around
observed zero-cooling minima; `refine_grid.py` runs72 cases and repeats both
reference and selected minima for each model/group. Full grid rechecks are
available with `--check`, but only their selected minima were replayed here.

`driver_followup.py` uses separately pinned valid physical driver resources after
the primary policy-label cases fell back to the default V9 driver. Four follow-up
outcomes repeat exactly. Different physical driver IDs also change their seeded
noise streams, so this is not an isolated derivative of aggressiveness.

The archived primary/supplement hashes, not this Mac's timing or current source
bytes, are checked in CI. Four cheap integrity tests do not rerun this campaign
on Linux or claim cross-platform physical digest parity. `plot_report.py` (optional
Matplotlib) renders only checksum-checked archives. See [next steps](NEXT_STEPS.md).

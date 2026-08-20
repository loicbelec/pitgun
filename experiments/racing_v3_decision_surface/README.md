# Racing Model V3 decision-surface screen

This experiment is the bounded local gate for the multi-era physical decision
audit tracked by [#244](https://github.com/loicbelec/pitgun/issues/244). It uses
the offline-only `pitgun.racing-v3-candidate@0.9.0`; it cannot change the game,
catalog `LATEST`, Authority, Verifier, or opponent policy.

The screen deliberately separates two questions:

1. do the new physical controls actually affect the result?
2. do gameplay decisions produce different useful optima by circuit?

The current V5 screen executes 882 deterministic three-lap simulations across
Monza, Monaco and Suzuka with seeds `7`, `42`, and `99`. The plan covers:

- a fixed forty-point development simplex for engine, cooling, aero and chassis;
- low/high downforce and gearing plus their five-by-five interaction grid;
- isolated V3 development-resolution, aerodynamic-resolution, target-speed
  transmission, braking, driveline, tire, driver, fuel-mass and
  compound-degradation controls.

The report binds `pitgun.racing-v3-candidate@0.9.0` and references the
checksummed 236-point, 6,144-lap evidence from
`experiments/racing_v3_tire_degradation/`. It does not repeat those long runs:
the short screen answers whether each control is observable, while the linked
campaign answers whether compound and strategy behavior survives race-length
state evolution.

Build and run from the repository root:

```bash
cargo build --release -p pitgun-racing-simulator \
  --example v3_decision_surface_probe
python3 experiments/racing_v3_decision_surface/screen_local.py --jobs 8
```

Use `--check` to prove that the stored report and adjacent SHA-256 file replay
byte for byte with the same binary and inputs. Parallel job count affects only
wall-clock time; points are sorted before canonical JSON serialization.

The probe accepts one resolved scenario, one versioned V3 experiment profile,
and one seed. Its output binds the candidate model, runner, scenario, profile,
and execution by digest while retaining setup, tire and mechanical diagnostics.
The profile is a Rust-only experiment boundary: absent overrides preserve the
candidate resolver, while a named override changes exactly one resolved Solver
input.

The immutable V1 report retains the mechanical candidate `0.3.0`; V2 retains
the aerodynamic-efficiency candidate `0.4.0`; V3 retains the
development-resolution candidate `0.5.0`; the V4 profile and report bind the
transmission-resolution candidate `0.6.0`. The V5 profile preserves that
transmission resolution while binding zero-downforce and first-stint-tire
fidelity to candidate `0.7.0`; its governed replay lives in
`experiments/racing_v3_validation/`. The V6 profile adds explicit
power-dependent combustion fuel and lap-level mass lineage under candidate
`0.8.0`; its bounded validation lives in `experiments/racing_v3_fuel_mass/`.

The V7 profile adds compound-dependent baseline, workload, and thermal wear
under candidate `0.9.0`. Its 236-point local and Databricks-ready campaign lives
in `experiments/racing_v3_tire_degradation/`. The current local-screen V5 report
uses this V7 profile; screen-report and profile version numbers are independent.
Profiles V1–V6 retain their previous identities and behavior.

## Current V5 result

- all 882 short executions bind the same Model V3 `0.9.0` identity;
- Monza, Monaco and Suzuka retain three distinct downforce/gearing optima;
- every calibration parameter changes a reviewed observable except the maximum
  thermal-wear multiplier, which is a dormant safety cap in the nominal short
  screen;
- the linked long-run evidence reaches a thermal multiplier of about `1.56`,
  below the default `3.0` cap, orders wear as soft > medium > hard in all 32
  groups, and still selects lap 22 in all 16 stop-window groups;
- aero and chassis investment are beneficial on all three short-screen
  circuits, cooling investment is harmful at the fixed forty-point budget, and
  engine investment changes direction by circuit.

These are diagnostic results, not production calibration. Multi-era,
progression and held-out coverage continues in #283 before the governed full
Databricks campaign in #284.

The current transmission interpretation is documented in
[`RACING_V3_TRANSMISSION_RESOLUTION_V1.md`](../../docs/RACING_V3_TRANSMISSION_RESOLUTION_V1.md).

The current development interpretation is documented in
[`RACING_V3_DEVELOPMENT_RESOLUTION_V1.md`](../../docs/RACING_V3_DEVELOPMENT_RESOLUTION_V1.md).

The current aerodynamic-efficiency interpretation is documented in
[`RACING_V3_AERO_EFFICIENCY_V1.md`](../../docs/RACING_V3_AERO_EFFICIENCY_V1.md).
The preceding mechanical-screen interpretation remains in
[`RACING_V3_LOCAL_DECISION_SCREEN_V1.md`](../../docs/RACING_V3_LOCAL_DECISION_SCREEN_V1.md).

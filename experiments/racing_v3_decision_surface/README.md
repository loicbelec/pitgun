# Racing Model V3 decision-surface screen

This experiment is the bounded local gate for the multi-era physical decision
audit tracked by [#244](https://github.com/loicbelec/pitgun/issues/244). It uses
the offline-only `pitgun.racing-v3-candidate@0.3.0`; it cannot change the game,
catalog `LATEST`, Authority, Verifier, or opponent policy.

The screen deliberately separates two questions:

1. do the new physical controls actually affect the result?
2. do gameplay decisions produce different useful optima by circuit?

It executes 351 deterministic three-lap simulations across Monza, Monaco and
Suzuka with seeds `7`, `42`, and `99`. The plan covers:

- a fixed forty-point development simplex for engine, cooling, aero and chassis;
- low/high downforce and gearing plus their coarse interaction grid;
- isolated V3 braking, transmission, driveline, tire and driver controls.

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

The reviewed interpretation is documented in
[`RACING_V3_LOCAL_DECISION_SCREEN_V1.md`](../../docs/RACING_V3_LOCAL_DECISION_SCREEN_V1.md).


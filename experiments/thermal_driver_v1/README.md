# Thermal driver V1 — local qualification

Native-only ability/commitment/correction/hesitation candidate. The historical
[resolution audit](../opponent_physics_v2/DRIVER_AUDIT_V1.md) remains evidence of
the original defect. [Equations and driver recap](../../docs/physics/THERMAL_DRIVER_V1.md).

`profile-v1.json` and `plan-v1.json` were committed before measurement. The first
pass revealed that scaling only longitudinal remainder did not reserve the
combined tire-force circle correctly; retain its failures. The corrected source
uses the same policy and cases, not newly fitted coefficients or looser tolerances.

The56-case plan covers four catalog archetypes × three modes on Monza5laps and
Barcelona20laps (24), one-trait risk-disabled pairs and reference modes (9),
Monza53laps × three seeds × three modes × risk on/off (18), and five frozen V2
physical regressions. Three seeded full-GP cases also compare normal execution
with unchanged pre-service continuation. Two whole passes must match exactly.
An additional22 one-car historical/energy outcomes are compared against their
frozen digests/errors; the prior grid10 case is explicitly outside this screen.

This is a causal/mechanical debug qualification, **not opponent calibration**,
reserved validation, expected win-rate estimation or rare-accident statistics.
No track/setup/coefficient fitting, hosted/provider call, browser manipulation,
WASM artifact, catalog/runtime admission or deployment is part of this run.

Reproduction from a clean committed tree:

```sh
cargo build --offline --locked --release -p pitgun-racing-simulator \
  --example thermal_driver_probe --example longitudinal_energy_probe
python3 experiments/thermal_driver_v1/qualify_local.py \
  target/release/examples/thermal_driver_probe \
  target/release/examples/longitudinal_energy_probe /tmp/pitgun-driver-qualification
```

The destination must not already exist. The runner stores compact receipts as
each case completes, including failures, source/policy/resource/input/result
identities and binary hashes. It never stores full telemetry here. Operational
wall time is not included in equality checks or claimed as a performance result.
See `RESULTS_V1.md` and `results/qualification-v1.json` for the final measured
outcome; those files are added after qualification.

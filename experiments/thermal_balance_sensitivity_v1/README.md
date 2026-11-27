# Thermal balance sensitivity V1

Tracked in [#486](https://github.com/loicbelec/pitgun/issues/486), following [the stop diagnostic](../era2_stop_diagnostic_v1/INTERPRETATION.md). Read [ACCEPTANCE.md](ACCEPTANCE.md) before running anything. All source policies and prior evidence remain intact. These trial coefficients are not selected by any game runtime.

## Model audit

The existing solver debits mechanical rolling/deformation work before distributing it into tire heat and explicit direct environmental dissipation. In `thermal_physics.rs`, the two nodes receive:

- surface power: deformation surface share × retained deformation power;
- carcass power: retained rolling power + remaining retained deformation power;
- signed surface/carcass, surface/air, surface/road and carcass/air exchanges.

Surface air conductance is `Hidle + Hspeed × speed`. Rolling work is `Crr × normal load × distance`; normal load includes vehicle weight and aerodynamic load. The prior developed-car Monza/São Paulo MH-one runs dissipate mean tire heat power about 14.1/13.5 kW, versus 7.5/7.3 kW in their era-II entry controls. Increased aerodynamic load and speed therefore expose the limits of a calibration based on lower-load cars. This does not establish that the work ledger is wrong or that any trial coefficient is a measured physical correction.

The three trials separately test retained rolling heat, surface-air exchange and carcass-air exchange. Neither mechanical resistance nor the temperature-to-grip window is changed. Changing temperature still changes grip and thermally accelerated wear through the existing laws. The source change is a separate observation probe that resolves actual career fixtures and measures hot/cold exposure, grip and thermal state; solver equations remain untouched.

## Reproduce the bounded experiment

Use clean pinned sources and fresh absolute phase cache directories outside the checkout. The native source commit and all source/binary/resource hashes are retained in the phase archives. One native process runs at a time; compilation uses two jobs. Do not mix earlier campaign caches with this experiment.

```sh
python3 -m unittest discover -s experiments/thermal_balance_sensitivity_v1 -v
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/ABSOLUTE/TARGET cargo test --offline --release -p pitgun-racing-simulator --example thermal_balance_sensitivity_probe
python3 experiments/thermal_balance_sensitivity_v1/run.py --phase development --target-dir /ABSOLUTE/TARGET --cache /ABSOLUTE/CAMPAIGN/development --output /ABSOLUTE/CAMPAIGN/development.json.gz
python3 experiments/thermal_balance_sensitivity_v1/report.py /ABSOLUTE/CAMPAIGN/development.json.gz --output-dir /ABSOLUTE/CAMPAIGN/frozen-selection
```

If `selection-v1.json` selects no eligible policy, stop. Otherwise preserve the selection and development hashes before reserved validation. Validation pins that exact selection; editing it requires a fresh namespace and does not authorize another candidate after looking at the reserved outcomes.

```sh
python3 experiments/thermal_balance_sensitivity_v1/run.py --phase validation --target-dir /ABSOLUTE/TARGET --cache /ABSOLUTE/CAMPAIGN/validation --output /ABSOLUTE/CAMPAIGN/validation.json.gz --selection /ABSOLUTE/CAMPAIGN/frozen-selection/selection-v1.json --development /ABSOLUTE/CAMPAIGN/development.json.gz
python3 experiments/thermal_balance_sensitivity_v1/report.py /ABSOLUTE/CAMPAIGN/development.json.gz --validation /ABSOLUTE/CAMPAIGN/validation.json.gz --output-dir /ABSOLUTE/CAMPAIGN/final-report
```

The maximum is 40 development races +8 reserved races +6 fresh repeats. Caches checkpoint every case and retain failures/accepted prefixes. Completed output is required for selection; all policies, including rejected trials, remain in the results. The read-only report recomputes selection from development only, checks the frozen selection against validation, and verifies full-result/physical-input repeat digests. It cannot promote a different policy based on reserved results.

Screening a candidate successfully is a prerequisite to broader era-I/II qualification, not an automatic model/catalog release or an AI calibration. No provider, Authority/Verifier, leaderboard or deployed service is called.

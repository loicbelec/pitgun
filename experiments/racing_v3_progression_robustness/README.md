# Racing Model V3 progression and held-out robustness

This bounded campaign audits the frozen `pitgun.racing-v3-candidate@0.9.0`
before any aero, chassis, cooling or engine calibration. It changes no model,
catalog or opponent policy.

The protocol deliberately separates three calibration circuits (Monza,
Monaco and Suzuka) from four held-out circuits (Spa, Barcelona, Singapore and
Mexico). It crosses all four active physical vehicles with the economy-backed
early, middle and late development budgets of 4, 27 and 37 points.

At each vehicle, circuit and progression anchor, the audit first applies direct
one-point additions and removals on every axis. These establish whether a
control is active, helpful or harmful. It then compares the balanced allocation
with every directed point transfer between the four development axes. The
transfer is one point at the early boundary, three at mid progression and four
at late progression; total budget never changes inside those comparisons.
Together, both screens expose marginal utility, interactions, saturation,
inactive controls and universal dominance without confusing specialization
with extra resources.

A separate 5 × 5 downforce/gearing grid at the middle budget measures setup
optimum diversity and neutral-setup regret. Long-run generic-strategy regret is
linked by digest from the compound campaign in
`experiments/racing_v3_tire_degradation/`; those expensive executions are not
duplicated.

## Run locally

```bash
cargo build --release -p pitgun-racing-simulator --example v3_decision_surface_probe
python3 -m unittest discover -s experiments/racing_v3_progression_robustness/tests -v
python3 experiments/racing_v3_progression_robustness/audit_local.py --jobs 8
python3 experiments/racing_v3_progression_robustness/audit_local.py --jobs 8 --check
```

The campaign contains 4,928 deterministic executions and 14,784 simulated
laps. The canonical report and its SHA-256 sidecar live in `results/`. Every
vehicle/progression anchor receives an explicit `PASS`, `REFINE`, or
`STRUCTURAL_CHANGE_REQUIRED` verdict, while calibration and held-out results
remain distinguishable in the stored evidence.

## V1 findings

The complete run succeeds for all 4,928 keys under the exact Model V3 `0.9.0`
identity. Direct one-point margins establish the following boundary:

- aero, chassis and engine are active on every reviewed vehicle and progression
  anchor;
- cooling is inactive on both historical V8 resources at all three budgets;
- cooling is useful at four points on `modern_v6t` and `f1_2026`, then produces
  exactly zero direct pace effect at 27 and 37 points;
- no axis is directly and universally harmful.

The constant-budget transfers reveal the gameplay consequence. Cooling points
are the usual donor on historical vehicles. On `f1_2026`, cooling is universally
dominant at the early anchor, while chassis becomes universally dominant at the
middle and late anchors. This discontinuity is classified
`STRUCTURAL_CHANGE_REQUIRED`: tuning scalar gains alone would move the cliff,
not create a progressive thermal trade-off.

The setup surface passes its held-out specialization gate. The middle-budget
5 × 5 grid exposes eight distinct optima on calibration circuits and ten on
held-out circuits. `classic_v8_1960` consistently selects the shortest gearing;
its downforce slider is intentionally non-discriminating because the governed
vehicle has body drag but no aerodynamic downforce. The other three resources
produce multiple circuit-specific downforce/gearing optima.

The linked long-run evidence remains `REFINE`: a generic lap-16 stop loses a
median 8,572 ms against the reviewed optimum, but lap 22 is still universally
fastest in all 16 circuit/vehicle groups. This audit therefore authorizes no
catalog, opponent-policy or production change. Its purpose is to define the
parameter and structural work that the governed Databricks campaign must test.

# Racing Model V3 Unified Decision Screen V1

Status: local evidence for #282 and the first delivery gate of #244. This
document does not select production coefficients, publish a catalog, or define
Opponent Policy V3.

## Question

The historical decision-surface screen proved that the first mechanical Model
V3 slices were observable, but it stopped at candidate `0.6.0`. Fuel and mass,
active-vehicle fidelity, first-stint tire state, and compound degradation were
added afterwards. The V5 screen asks whether the complete short-form decision
surface still behaves coherently under the exact
`pitgun.racing-v3-candidate@0.9.0` identity.

It deliberately combines two immutable evidence layers instead of creating an
unbounded Cartesian product:

1. 882 deterministic three-lap executions screen development, setup and thirty
   physically named profile axes across Monza, Monaco and Suzuka with seeds 7,
   42 and 99;
2. the checksummed #280 report contributes 236 long-run executions and 6,144
   simulated laps across four circuits and four governed vehicle identities.

The short screen measures activation and direction. The long screen measures
state accumulation, compound crossovers and one-stop timing. Both bind Model V3
`0.9.0`; neither changes the game or a hosted verification identity.

## Reproducibility

From the repository root:

```bash
cargo build --release -p pitgun-racing-simulator \
  --example v3_decision_surface_probe
python3 experiments/racing_v3_decision_surface/screen_local.py --jobs 8
python3 experiments/racing_v3_decision_surface/screen_local.py --jobs 8 --check
```

The generated report and adjacent checksum are:

- `experiments/racing_v3_decision_surface/results/local-screen-v5.json`;
- `experiments/racing_v3_decision_surface/results/local-screen-v5.sha256`.

Parallelism changes wall-clock time only. Points are canonically ordered before
serialization. The report verifies the SHA-256 of the long-run artifact before
including its compact lineage.

## Results

### Parameter activation

Fuel-consumption parameters and the compound-degradation coefficients are now
visible in the same report as the established mechanical axes. For example:

- brake-specific fuel consumption changes three-lap fuel consumption by about
  2.09 to 3.65 kg across the reviewed circuits;
- the degradation reference-load coefficient changes final tire wear by about
  1.67 to 2.97 percentage points;
- thermal-deviation gain changes both requested wear and elapsed time.

The maximum thermal-wear multiplier is intentionally dormant in the nominal
three-lap screen. The linked long-run campaign observes a maximum multiplier of
about 1.56, below the default safety cap of 3.0. This is reported as an inactive
bound, not silently interpreted as a useful calibration direction.

### Setup

The coarse downforce/gearing surface retains a different optimum for every
representative circuit:

| Circuit | Reviewed optimum |
| --- | --- |
| Monza | downforce `0.5`, gearing `0.75` |
| Monaco | downforce `1.0`, gearing `0.0` |
| Suzuka | downforce `1.0`, gearing `0.75` |

This passes the first circuit-specificity gate. It does not yet prove held-out
robustness or multi-era setup diversity.

### Development allocation

At the fixed forty-point budget:

- more aero is faster on all three reviewed circuits;
- more chassis is faster on all three reviewed circuits;
- more cooling is slower because the short screen does not reach a thermal
  derating regime worth the points removed from other axes;
- the engine direction changes by circuit.

The former single chassis direction is no longer the whole surface, but the
result remains `REFINE`: universal aero/chassis benefit and universally harmful
cooling may still produce simple player allocations at the reviewed horizon.
The next campaign must distinguish short-horizon opportunity cost from
race-length thermal value and evaluate progression anchors.

### Tires and strategy

The linked #280 evidence confirms:

- soft > medium > hard accumulated wear in all 32 compound groups;
- all 64 softer-versus-harder comparisons cross within 24 laps;
- every one of the 16 reviewed one-stop groups selects lap 22.

The degradation law is therefore connected and explorable, but its current
compound pace/wear balance is not calibrated for strategy diversity.

## Decision

The current candidate earns three conclusions:

1. `PASS` for circuit-dependent setup on the three calibration circuits;
2. `REFINE` for parameter activation because the thermal safety cap is outside
   the nominal activation domain;
3. `REFINE` for development and strategy diversity.

No structural rollback is justified. #283 should now extend the exact model to
active vehicle classes, progression anchors and held-out circuits. Only that
accepted local plan should become the governed Databricks campaign in #284.


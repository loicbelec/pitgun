# Racing Model V3 thermal Databricks plan V1

Status: immutable execution contract for #287 and #293. It authorizes no
coefficient or production promotion.

## Decision to make

The campaign asks whether the current one-node engine-thermal family can be
made safe, progressive, era-aware, and strategically useful through bounded
coefficient calibration. It must distinguish that outcome from a case where
the equation shape itself needs to change.

The campaign does not minimize lap time. A cold engine that never exercises
thermal behavior and an overheated engine for which maximum cooling is always
dominant are both inadequate outcomes.

## Three evidence partitions

| Partition | Purpose | Inputs |
|---|---|---|
| local replay | prove the packaged Linux/ARM64 Rust probe reproduces retained local evidence | the 15 engaged healthy sets, all enabled vehicle anchors, Monza and Singapore, cooling 0/10/20, seeds 42 and 99 where reviewed |
| transition densification | inspect the narrow boundary between healthy and pathological behavior | eight deterministic interpolations between the four healthy and four hot refinement anchors |
| final validation | test transfer without reusing selection inputs | Barcelona and seed `20260821`, absent from the local adaptive campaign, with short and long workloads |

The immutable manifest therefore plans 1,560 executions:

- 720 exact local replay executions;
- 288 transition-densification executions;
- 552 reserved validation executions;
- 23 thermal profiles and 126 resolved scenarios.

Local calibration and held-out points remain labelled as replay evidence. They
cannot become independent validation merely because they run on Databricks.

## Adequacy contract

The numerical guards classify experiment output; they are not claimed as
measured Formula 1 calibration targets.

Every family must remain below `180 °C`, below 50% derated duration, and return
finite diagnostics. Historical V8s are not required to exhibit modern thermal
management: they must remain safe and must not gain a universal pace advantage
from maximum cooling. `modern_v6t` and `f1_2026` must additionally demonstrate
long-run thermal engagement and a progressive cooling trade-off which is
neither globally inactive nor universally dominant.

The reviewed output for each vehicle family is one of:

- `PASS`: the one-node family has a robust coefficient region;
- `REFINE`: the family is viable but the governed region requires more bounded
  sampling or era-specific authored coefficients;
- `STRUCTURAL_CHANGE_REQUIRED`: no robust parameter region satisfies the
  declared behavior, so a richer equation must be designed.

## Reproduction

Rebuild the manifest from the checksummed local report:

```bash
python3 experiments/databricks/build_v3_thermal_manifest.py
python3 -m unittest \
  experiments.databricks.tests.test_v3_thermal_surface_campaign
```

The builder freezes content-addressed scenarios, profiles, natural keys,
lineage, and exact Rust evidence for replay points. New transition and final
validation points deliberately have no expected result: Databricks creates new
evidence for them, while their inputs remain immutable.

## Governance

Rust remains the sole evaluator of physical equations. Python constructs and
validates the explicit plan; Databricks orchestrates compute and stores Delta
and MLflow evidence. The campaign cannot publish a catalog, alter the game,
change Authority or Verifier compatibility, activate an energy controller, or
regenerate an opponent policy.

Execution and read-only review are the next delivery slice. Only the reviewed
per-era verdict may authorize either a candidate parameter profile or a new
thermal equation proposal.

The completed evidence and conservative per-family verdicts are recorded in
[Racing Model V3 thermal Databricks review V1](RACING_V3_THERMAL_DATABRICKS_REVIEW_V1.md).

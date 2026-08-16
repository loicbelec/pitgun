# Racing early-allocation effect V1

## Decision

The governed campaign selects no opponent allocation profile and promotes no
catalog or game change. Its ranking describes the current model response at a
single four-point early-game boundary; copying it into AI weights would make
opponents exploit that local response instead of becoming more coherent.

Opponent Policy V2 may still publish bounded authored styles for explainable
field diversity. The physical meaning, saturation, and progression mapping of
the four development axes remain governed by issue #229.

## Protocol

The immutable campaign changes one player development point on one named axis
at a time. It crosses five physical circuits (Budapest, Monaco, Monza,
Singapore, and Suzuka), three seeds, one neutral reference, four add-one
treatments, and four remove-one treatments: 135 deterministic runs in total.

The successful execution produced 60 paired axis comparisons. All 20
circuit/axis groups retained the same direction over all three seeds.

## Evidence

Median marginal benefit is computed from total race time. Positive values mean
that adding a point is faster and removing it is slower.

| Axis | Add minus reference | Remove minus reference | Marginal benefit per point |
| --- | ---: | ---: | ---: |
| Chassis | -10,021 ms | +10,086 ms | +10,053.5 ms |
| Engine | -468 ms | +455 ms | +468.0 ms |
| Cooling | 0 ms | 0 ms | 0.0 ms |
| Aero | +351 ms | -361 ms | -356.5 ms |

The circuit-level marginal benefit remained directionally stable:

| Circuit | Aero | Chassis | Cooling | Engine |
| --- | ---: | ---: | ---: | ---: |
| Budapest | -356.5 ms | +11,195.0 ms | 0.0 ms | +468.0 ms |
| Monaco | -266.5 ms | +10,053.5 ms | 0.0 ms | +430.0 ms |
| Monza | -686.0 ms | +7,490.0 ms | 0.0 ms | +525.5 ms |
| Singapore | -314.5 ms | +11,034.5 ms | 0.0 ms | +482.5 ms |
| Suzuka | -457.5 ms | +9,838.5 ms | 0.0 ms | +417.5 ms |

## Interpretation

The observations reconcile with the versioned implementation:

- chassis development directly increases the tire-grip coefficient;
- engine development increases the complete torque curve;
- cooling only changes pace after the thermal model reaches derating, which
  did not happen in these neutral scenarios;
- aero development scales downforce and drag together, and the extra drag is
  slightly more costly at this boundary.

These are causal statements about the controlled scenarios, not universal
axis values. Interactions between simultaneous investments, other budgets,
other circuits, and thermal stress remain outside this campaign.

## Lineage

- campaign: `racing-early-allocation-effect-2026-v1`
- manifest: `sha256:b590f0894251616665f8a4ca7ee2cddc3d142c4babfb580e60a8792b5fc9f989`
- complete Databricks run: `751693981476198`
- idempotent replay: `171282935787022`
- MLflow run: `5668a48da19a4263828d3ef712addd0f`
- runner artifact: `sha256:17fd5ffdfe15a64a3c6dc3599d5f36e6677b735c530487d0e20baad046e86e01`
- successful source revision: `54a60dad3d27`

The complete run succeeded for 135/135 keys with zero invalid or failed runs.
The replay attempted zero simulations and reused all 135 accepted keys.


# Databricks Circuit Sweep V1

Status: immutable campaign inputs and local contract implemented for issue
#166; observed workspace evidence is recorded only after the attended run.

## Question

Which bounded setup families are fast and robust across representative Racing
circuit archetypes, and where does the preferred setup materially change?

The campaign is the second calibration increment. The first nine-run reference
campaign proved the execution and publication chain on Monza. This sweep adds
circuit coverage before Pitgun designs multi-lap strategies or changes the
game's opponents.

## Controlled matrix

The immutable manifest contains 35 exact resolved scenarios and three seeds,
for 105 planned deterministic runs.

| Archetype | Physical circuit model | Circuit |
|---|---|---|
| Power | `it-1922` | Monza |
| High downforce | `mc-1929` | Monaco |
| Mechanical grip | `hu-1986` | Budapest |
| Mixed | `jp-1962` | Suzuka |
| Street / thermal | `sg-2008` | Singapore |

The seven setup families retain the reference low, balanced, and high centers,
then decouple downforce from gearing so the analysis can observe interactions:

| Family | Downforce | Gear-ratio slider |
|---|---:|---:|
| `low-downforce` | 0.2 | 0.3 |
| `balanced` | 0.5 | 0.5 |
| `high-downforce` | 0.8 | 0.7 |
| `low-downforce-long-gearing` | 0.2 | 0.7 |
| `high-downforce-short-gearing` | 0.8 | 0.3 |
| `balanced-short-gearing` | 0.5 | 0.3 |
| `balanced-long-gearing` | 0.5 | 0.7 |

Every scenario uses the controlled `f1_2026` vehicle, equal development
points, one lap, and no pit stop. This isolates setup/circuit interaction;
strategy calibration is deliberately a later campaign.

## Trust and reproducibility

[`racing-circuit-sweep-v1.json`](../experiments/databricks/campaigns/racing-circuit-sweep-v1.json)
and its checksum freeze the complete plan. Each row binds the exact scenario
resource, expected configuration identity, scenario digest, circuit, setup,
and strategy. The wheel packages all 35 reviewed JSON resources. Its adapter
accepts only canonical embedded resource identifiers and cannot load a path,
URL, notebook-provided model, or player setup.

The natural key remains `(campaign_id, configuration_id, seed)`. Accepted rows
are skipped on retry; invalid and failed attempts remain auditable. Delta owns
the governed run and metric history, while MLflow owns the campaign parameters,
report, and circuit/setup comparison plot.

## Execution

After repository review and a fresh Linux/arm64 wheel build:

```bash
cd experiments/databricks
databricks bundle validate -t dev -p pitgun-free --strict
databricks bundle plan -t dev -p pitgun-free
databricks bundle deploy -t dev -p pitgun-free
databricks bundle run circuit_sweep_job -t dev -p pitgun-free
```

Running the job a second time is the required idempotency check. It must retain
the same 105 accepted natural keys and MLflow run identity while attempting no
new simulation.

## Observed execution

The first Free Edition execution completed on 2026-08-12 with all 105 planned
runs accepted, no invalid input, and no execution failure. The campaign-level
executor took 78,566 ms. The complete two-task job took approximately six
minutes, with serverless startup and bootstrap again dominating the native
simulation time.

The immediate retry preserved the same 105 accepted natural keys and MLflow
run `87165294f0b3480d868d2a1a0ee9b346`. It attempted zero simulations, skipped
all 105 accepted rows, and reconciled the campaign in 13,854 ms. The complete
serverless job still took approximately five minutes, confirming that Free
Edition orchestration latency dominates idempotent workloads.

| Circuit | Fastest tested setup | Mean lap | Advantage over same-downforce alternate | Tested pace spread |
|---|---|---:|---:|---:|
| Monza (`it-1922`) | `high-downforce-short-gearing` | 83,789.33 ms | 80.33 ms | 3,089.33 ms |
| Monaco (`mc-1929`) | `high-downforce-short-gearing` | 64,267.67 ms | 104.00 ms | 2,836.67 ms |
| Budapest (`hu-1986`) | `high-downforce-short-gearing` | 77,721.00 ms | 98.00 ms | 4,345.67 ms |
| Suzuka (`jp-1962`) | `high-downforce-short-gearing` | 88,991.67 ms | 70.33 ms | 4,378.67 ms |
| Singapore (`sg-2008`) | `high-downforce-short-gearing` | 88,154.00 ms | 124.00 ms | 3,356.00 ms |

Seed dispersion remained tightly bounded: every configuration observed a lap
range of 71 or 72 ms and a population standard deviation between 30.55 and
31.02 ms. The result is therefore reproducible, not random noise.

The main finding is negative but decisive: the five editorial circuit
archetypes did **not** produce different preferred setups within this response
surface. Maximum tested downforce with short gearing won everywhere. On Monza,
long gearing raised observed maximum speed, but still lost lap time. A
circuit-aware policy derived now would encode cosmetic variety rather than an
evidence-backed optimum.

Issue #169 therefore owns the next investigation: measure intermediate tuning
effects and physical circuit descriptors, distinguish a narrow parameter grid
from a model-response defect, and prove circuit-dependent trade-offs before
policy V2 in #167.

### Reproducible lineage

- campaign: `racing-circuit-sweep-2026-v1`;
- manifest digest: `sha256:df9fb6356095f91ab9e919e722a03d5ebf564880b10686d6af7d68d1fca6b86d`;
- source revision: `a2d625a958d2`;
- MLflow run: `87165294f0b3480d868d2a1a0ee9b346`;
- first Databricks job run: `180235537401535`;
- idempotency job run: `415097636928935`.

## Boundaries

This campaign does not yet:

- calibrate full-race stint or pit-stop strategy;
- cover every catalog circuit or progression era;
- consume player, career, leaderboard, or private telemetry data;
- publish a policy or change `latest.json`;
- make Databricks a runtime dependency of the browser game.

Its reviewed results are the input to issue #167, which owns the
game-compatible nine-opponent policy and explicit fallback behavior.

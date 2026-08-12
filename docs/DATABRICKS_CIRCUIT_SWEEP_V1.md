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

## Boundaries

This campaign does not yet:

- calibrate full-race stint or pit-stop strategy;
- cover every catalog circuit or progression era;
- consume player, career, leaderboard, or private telemetry data;
- publish a policy or change `latest.json`;
- make Databricks a runtime dependency of the browser game.

Its reviewed results are the input to issue #167, which owns the
game-compatible nine-opponent policy and explicit fallback behavior.

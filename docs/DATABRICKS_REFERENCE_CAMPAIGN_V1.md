# Databricks Reference Campaign V1

Status: immutable inputs and idempotent Delta/MLflow execution implemented for
issue #156; observed campaign evidence is recorded after the first live run.

## Question

The campaign compares three deliberately simple setup families on the 2026
`it-1922` reference circuit. Each family runs with three explicit seeds, for
nine planned deterministic runs.

| Family | Downforce | Gear ratio | Strategy |
|---|---:|---:|---|
| `low-downforce` | 0.2 | 0.3 | single lap, no stop |
| `balanced` | 0.5 | 0.5 | single lap, no stop |
| `high-downforce` | 0.8 | 0.7 | single lap, no stop |

This is a calibration-pipeline reference workload, not yet a complete opponent
balance suite. It proves multiple setup families, seed robustness, governed
execution, and comparable evidence before Pitgun introduces multi-lap strategy
families.

## Immutable manifest

[`racing-reference-v1.json`](../experiments/databricks/campaigns/racing-reference-v1.json)
is the execution manifest. Its companion SHA-256 file pins its exact bytes.
Changing a seed, family, expected identity, or question requires a new manifest
version and campaign identifier; a running campaign is never edited in place.

The nine planned rows are the Cartesian product of the three allowlisted
configuration families and seeds `42`, `2026`, and `20260810`. Every family
binds its expected `configuration_id` and `scenario_digest`, computed by the
Rust runner. Execution must fail closed if either identity differs.

## Execution boundary

The Databricks wheel embeds the three complete resolved scenarios. The adapter
accepts a family identifier from a fixed allowlist and never accepts a path,
URL, arbitrary command, or partial physics override. This preserves the
security property established by the serverless packaging spike while allowing
the campaign orchestrator to select among governed configurations.

## Governed execution

`reference_campaign_job` first applies additive schema evolution, then executes
only natural keys that do not already have an accepted result. The natural key
is `(campaign_id, configuration_id, seed)`. A retry may replace a failed or
invalid attempt with a successful result, but it cannot duplicate or overwrite
an accepted run.

Every run row binds the manifest, family, seed, scenario, model, data pack,
adapter Git revision, CLI version, native runner digest, and compact-result
digest. Identity mismatches are stored as `INVALID`; execution exceptions are
stored as `FAILED`. Planned, successful, invalid, and failed counts must
reconcile before the campaign can finish.

Successful runs contribute lap time, maximum speed, and telemetry frame-count
rows to the governed metrics table. MLflow resumes one stable run for the
campaign and records inputs, terminal counts, per-family mean pace, seed
dispersion, speed, overall family pace spread, duration, a JSON report, and an
SVG comparison of mean lap time with the observed three-seed range.
MLflow parameters and the input manifest are logged only when that tracking run
is created; retries resume it without attempting to rewrite immutable lineage.

Run from `experiments/databricks`:

```bash
databricks bundle deploy -t dev -p pitgun-free
databricks bundle run reference_campaign_job -t dev -p pitgun-free
```

Running the job again is the explicit idempotency check: all nine accepted runs
must be skipped while the Delta row counts and MLflow run identity remain
unchanged.

## Observed execution

The first Free Edition campaign completed on 2026-08-11 with all nine planned
runs accepted, no invalid input, and no execution failure. The campaign-level
orchestrator took 27,971 ms; the complete two-task Databricks job took 286,506
ms, including 185,000 ms of cold serverless setup and 44,000 ms of bootstrap
task execution.

| Family | Mean lap | Seed std. dev. | Range | Mean maximum speed |
|---|---:|---:|---:|---:|
| `high-downforce` | 83,869.67 ms | 30.71 ms | 71 ms | 351.92 km/h |
| `balanced` | 85,281.33 ms | 30.55 ms | 71 ms | 355.57 km/h |
| `low-downforce` | 86,807.00 ms | 31.02 ms | 72 ms | 359.04 km/h |

The family pace spread was 2,937.33 ms. On this fixture, maximum straight-line
speed alone is therefore a poor selection objective: the low-downforce family
was fastest in that metric and slowest over the lap.

The immediate retry completed with the same nine successful Delta rows and the
same MLflow run identity. It attempted zero simulations and skipped all nine
accepted natural keys. Its campaign-level reconciliation took 6,244 ms; the
complete serverless job still took 266,024 ms, including 205,000 ms of cold
setup. This is the main observed Free Edition limitation: orchestration latency
dominates tiny deterministic workloads even when compute cost is monetarily
zero.

The governed results subsequently produced the first immutable
[Racing Opponent Policy V1](RACING_OPPONENT_POLICY_V1.md). Selection reads
historical Delta versions rather than mutable latest state and preserves the
three materially distinct families as separate field roles.

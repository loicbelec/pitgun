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
dispersion, speed, overall family pace spread, duration, and a JSON report.

Run from `experiments/databricks`:

```bash
databricks bundle deploy -t dev -p pitgun-free
databricks bundle run reference_campaign_job -t dev -p pitgun-free
```

Running the job again is the explicit idempotency check: all nine accepted runs
must be skipped while the Delta row counts and MLflow run identity remain
unchanged.

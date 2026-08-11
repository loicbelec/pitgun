# Databricks Reference Campaign V1

Status: parameter space frozen for the first governed execution of issue #156.

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

The next increment materializes these nine rows, executes them idempotently,
persists structured successes and failures to Delta, and logs campaign-level
parameters and aggregates to MLflow.

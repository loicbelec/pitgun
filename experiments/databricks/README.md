# Pitgun Databricks experiments

This directory is the source-controlled integration root for offline Pitgun
calibration campaigns. The target architecture and acceptance criteria live in
[`docs/DATABRICKS_CALIBRATION_V1.md`](../../docs/DATABRICKS_CALIBRATION_V1.md).

## Status

This directory contains the Declarative Automation Bundle for Calibration V1.
It provisions only the dedicated `workspace.pitgun_calibration` and
`workspace.pitgun_policies` data plane, its MLflow experiment, and its
parameterized serverless bootstrap job. The existing `workspace.default`
objects remain outside the bundle and are never modified.

The initial workspace is Databricks Free Edition. Local attended development
uses the OAuth profile `pitgun-free`; that profile and its credentials belong to
the developer machine and must never be committed.

## Layout

```text
experiments/databricks/
├── databricks.yml
├── resources/
│   ├── experiments.yml
│   ├── jobs.yml
│   └── schemas.yml
├── src/
│   └── bootstrap_tables.py
└── README.md
```

Configuration is portable: no workspace URL, user identity, catalog storage
path, token, or secret belongs in the repository.

## Attended development

Run commands from this directory. The local OAuth profile selects the workspace
without becoming part of bundle configuration:

```bash
cd experiments/databricks
databricks bundle validate -t dev -p pitgun-free --strict
python3 -m unittest discover -s tests
databricks bundle plan -t dev -p pitgun-free
databricks bundle deploy -t dev -p pitgun-free
databricks bundle run bootstrap_job -t dev -p pitgun-free \
  --params operation=bootstrap,campaign_id=bundle-smoke
databricks bundle run runner_spike_job -t dev -p pitgun-free \
  --params seed=42
```

Repeat `deploy` and `run`: schema and table creation are idempotent. The job may
also use `operation=validate` to check the complete data plane without executing
DDL.

The job uses a declared serverless environment and no cluster identifier. Free
Edition quota exhaustion delays a run but does not change its logical result.

The target is named `dev`, but uses bundle `production` naming semantics. This
does not make the workspace a production service: it prevents development mode
from silently prefixing the stable Unity Catalog schema names. Bundle files and
state remain isolated below the attended user's `dev` deployment root.

## Packaged Rust runner

`adapter/build.sh` builds the existing Rust CLI for Linux/arm64 and places
it inside a platform wheel. The bundle uploads that wheel as the only custom
library of `runner_spike_job`. Generated binaries and wheels are ignored by Git.

The bounded adapter accepts only a seed, executes the embedded scenario, and
records the exact runner and result digests. See
[Databricks Rust Runner Spike](../../docs/DATABRICKS_RUST_RUNNER_SPIKE.md) for
the decision, local parity fixture, measurements, rejected alternatives, and
security boundary.

## Governed table ownership

The attended deployment identity owns both bundle-created schemas. The job run
identity owns the Delta tables it creates inside them. Every table has a SQL
comment and `pitgun.grain`, `pitgun.owner_domain`, and
`pitgun.contract_version` properties. V1 tables are:

| Table | Grain |
|---|---|
| `pitgun_calibration.campaigns` | one campaign |
| `pitgun_calibration.runs` | one campaign, materialized configuration, and seed |
| `pitgun_calibration.metrics` | one successful run and metric |
| `pitgun_calibration.candidates` | one reviewed candidate and difficulty band |
| `pitgun_policies.releases` | one immutable policy release version |

No personal or player data belongs in these V1 tables.

## Destruction boundary

`databricks bundle destroy -t dev -p pitgun-free` is destructive: it attempts
to remove resources owned by this deployment. Non-empty Unity Catalog schemas
may first require an explicit, separately reviewed table-retirement operation.
Always inspect `databricks bundle plan` and retain approved policy artifacts
elsewhere before destruction.

The command cannot remove the legacy `workspace.default` assets because those
objects are neither declared nor bound to this bundle. Destruction must remain
an explicit human operation; it is never part of CI or a campaign job.

## Delivery sequence

1. #153 defines and implements the local runner.
2. #154 provisions governed resources as a bundle.
3. #158 proves the Rust execution adapter on serverless compute.
4. #156 runs the reference calibration campaign.
5. #155 publishes the reviewed policy artifact.
6. #157 documents the real result publicly.

## Workspace safety

Legacy assets in `workspace.default` are read-only inputs during V1. Bundle
deployment must use the dedicated `pitgun_calibration` and `pitgun_policies`
schemas and must not rename, migrate, or delete old tables and notebooks.

Bundle destruction may remove only resources created and owned by the bundle.
Catalog publication remains a separately reviewed human action.

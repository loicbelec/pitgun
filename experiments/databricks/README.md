# Pitgun Databricks experiments

This directory is the source-controlled integration root for offline Pitgun
calibration campaigns. The target architecture and acceptance criteria live in
[`docs/DATABRICKS_CALIBRATION_V1.md`](../../docs/DATABRICKS_CALIBRATION_V1.md).

## Status

The directory currently records the delivery boundary only. It does not yet
contain a deployable Databricks bundle. Issue #154 adds the bundle after issue
#153 defines the machine-readable Rust runner contract.

The initial workspace is Databricks Free Edition. Local attended development
uses the OAuth profile `pitgun-free`; that profile and its credentials belong to
the developer machine and must never be committed.

## Intended layout

```text
experiments/databricks/
├── databricks.yml
├── resources/
│   ├── experiments.yml
│   ├── jobs.yml
│   └── schemas.yml
├── src/
│   └── pitgun_calibration/
├── tests/
└── README.md
```

The exact layout may change during #154 when it is validated against the current
Declarative Automation Bundles schema. Configuration must remain portable: no
workspace URL, user identity, catalog storage path, token, or secret belongs in
the repository.

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

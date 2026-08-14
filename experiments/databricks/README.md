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
│   ├── bootstrap_tables.py
│   ├── execute_candidate_validation.py
│   ├── execute_reference_campaign.py
│   └── select_reference_opponent_policy.py
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
databricks bundle run reference_campaign_job -t dev -p pitgun-free
databricks bundle run reference_policy_job -t dev -p pitgun-free
databricks bundle run circuit_sweep_job -t dev -p pitgun-free
databricks bundle run candidate_validation_job -t dev -p pitgun-free
databricks bundle run candidate_review_job -t dev -p pitgun-free
databricks bundle run opponent_diagnosis_job -t dev -p pitgun-free \
  --params campaigns_table_version=<pinned>,runs_table_version=<pinned>,metrics_table_version=<pinned>
databricks bundle run strategy_effect_job -t dev -p pitgun-free
databricks bundle run budget_effect_job -t dev -p pitgun-free
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

`adapter/build.sh` rebuilds the Rust CLI and tuning-response probe separately
for Linux/arm64, then places both inside a platform wheel. Its persistent Cargo
cache is invalidated for those packages first so a Git checkout cannot leave a
stale runner in the wheel. The bundle uploads that wheel as the only custom
library of `runner_spike_job`. Generated binaries and wheels are ignored by Git.

The bounded adapter accepts only reviewed identifiers and a seed, executes an
embedded scenario, and records the exact runner and result digests. The Racing
V2 boundary also packages Catalog 1.2.0 and the controlled opponent-audit smoke
matrix in the wheel; it does not fetch models or scenarios over the network.
See
[Databricks Rust Runner Spike](../../docs/DATABRICKS_RUST_RUNNER_SPIKE.md) for
the decision, local parity fixture, measurements, rejected alternatives, and
security boundary.

The explicit opponent audit campaign is frozen in
[`campaigns/racing-opponent-audit-v1.json`](campaigns/racing-opponent-audit-v1.json).
It contains 180 planned Racing V2 runs and verifies the digest of every packaged
scenario before execution. Its circuit-informed player is a public-baseline
diagnostic, not a validated optimum or an automatically publishable policy.
`opponent_audit_job` bootstraps the additive run-lineage columns, resumes from
successful immutable run keys, executes the 180 packaged scenarios, and writes
normalized competitiveness evidence to the existing Delta `runs` and
`metrics` tables. One MLflow run stores the manifest and compact comparison
report. Deployment and execution remain separate attended commands.

`opponent_diagnosis_job` then reads only explicit historical versions of the
`campaigns`, `runs`, and `metrics` Delta tables. Its reviewed defaults pin
versions 19, 5, and 3 respectively: the completed 180-run campaign and its
idempotent replay. It does not rerun simulations or write a
policy. Its MLflow JSON and Markdown artifacts distinguish exact paired setup
effects from descriptive strategy, progression, budget, and diversity signals;
confounded comparisons are labelled and retained as unresolved questions.

The follow-up strategy-effect input campaign is frozen in
[`campaigns/racing-strategy-effect-v1.json`](campaigns/racing-strategy-effect-v1.json).
Its 45 pairs retain the neutral player tuning and the exact balanced-source
opponent field. Only the controlled player's `stint_strategy` differs inside a
pair. The manifest records both strategy digests and a common invariant digest;
the adapter validates all 90 packaged resources before exposing an execution
plan. This input-only delivery performs no compute and selects no policy.

`strategy_effect_job` executes or resumes those 90 immutable run keys on the
packaged Linux/ARM64 Racing runner. It appends only campaign, run, and metric
evidence to the governed Delta tables, then reports 45 exact late-minus-balanced
pairs by circuit and progression in MLflow. Three-seed direction stability and
the precise Delta, runner, manifest, adapter, and source Git lineage accompany
the result. Neither candidate nor policy tables are written.

The controlled development-budget input campaign is frozen in
[`campaigns/racing-budget-effect-v1.json`](campaigns/racing-budget-effect-v1.json).
It retains the same balanced opponent field and player strategy. For each of
five circuits, three progression stages, and three seeds, it sets the player's
development budget to 90%, 100%, or 110% of the frozen field median, allocated
deterministically across aero, chassis, cooling, and engine points.

The result is 45 exact triplets and 135 planned runs. Inside a triplet, only
the player's budget cap and balanced four-axis allocation may differ. The
adapter rejects any other change, any observed player data, or any attempt to
turn this input-only campaign into an automatic game-balance decision.

`budget_effect_job` executes or resumes those 135 immutable keys, stores the
normalized evidence in the governed Delta tables, and publishes exact
90%-minus-100% and 110%-minus-100% dose-response summaries to MLflow. A second
execution skips successful keys and proves idempotence. Incomplete triplets
remain visible but receive no causal interpretation, and no target budget is
selected or promoted.

## Reference campaign

The frozen V1 campaign manifest is
[`campaigns/racing-reference-v1.json`](campaigns/racing-reference-v1.json).
It plans nine deterministic executions across three embedded setup families
and three seeds. Its scope and current limitations are documented in
[Databricks Reference Campaign V1](../../docs/DATABRICKS_REFERENCE_CAMPAIGN_V1.md).
The campaign job bootstraps additive schema changes, skips accepted natural
keys on retry, merges compact run and metric evidence into Delta, and resumes
one campaign-level MLflow run.

The policy job reads explicitly pinned historical versions of the campaign,
runs, and metrics tables. It applies deterministic multi-objective constraints,
writes only `PROPOSED` candidate and release rows, and logs the exact policy to
the existing campaign MLflow run. It cannot approve or publish a catalog
release; that remains a protected repository review.

## Representative circuit sweep

The immutable
[`racing-circuit-sweep-v1.json`](campaigns/racing-circuit-sweep-v1.json)
manifest plans 105 executions: seven reviewed setup configurations, five
physical circuit archetypes, and three explicit seeds. Every one of its 35
resolved scenarios is embedded in the platform wheel and selected by a
canonical resource identifier; the job accepts no arbitrary scenario path,
URL, or physics override.

The sweep reuses the same Delta tables, MLflow experiment, idempotent natural
keys, and Rust runner as the reference campaign. It produces evidence for a
future game-compatible opponent policy but does not publish or promote one.

## Aerodynamic candidate validation

The immutable `racing-aero-candidate-validation-v1` campaign compares the
historical response with the calibrated aerodynamic candidate over the same
35 reviewed circuit/setup scenarios and three seeds: 210 executions. A
dedicated Rust probe and two allowlisted response resources are embedded in the
wheel; the job accepts neither arbitrary physics JSON nor an artifact URL.

Experimental executions deliberately use `experimental_execution_id` and the
dedicated `experimental_runs` and `experimental_metrics` tables. They never
claim a canonical `run_id`. MLflow records the immutable manifest, identities,
metrics, and review report. Even a successful campaign ends at
`REVIEW_REQUIRED`: changing the catalog or game remains a separate reviewed
repository change.

The separately deployed candidate-review job reads explicit Delta versions 1
of both experimental tables. Its versioned policy distinguishes setup
discrimination from seed noise, checks circuit-specific expected setup
families, and appends a `PROMOTE`, `REFINE`, or `REJECT` review artifact to the
same MLflow run. Even `PROMOTE` is advisory: the job contains no catalog write
or release path.

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
| `pitgun_calibration.experimental_runs` | one experimental response/configuration/seed execution |
| `pitgun_calibration.experimental_metrics` | one experimental execution and metric |
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

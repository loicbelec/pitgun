# Databricks Calibration V1

## Outcome

Databricks Calibration V1 turns deterministic Pitgun simulations into a
reproducible offline calibration loop. Racing is the first workload: the loop
must identify varied, fair opponent candidates for a circuit, era, and
difficulty band, then publish a versioned policy that the game can load without
recompilation.

Databricks is an experiment and data-engineering environment in this design. It
does not serve game traffic, issue simulation authorizations, verify leaderboard
submissions, or become a production dependency for browser races.

## V1 question

The first campaign answers one bounded question:

> Which deterministic setup and strategy families produce competitive but
> materially different opponents for one representative Racing circuit and era?

The reference campaign is deliberately smaller than the final balance suite. It
must prove the complete path from versioned inputs to a policy consumed by the
game before Pitgun scales across every circuit and era.

## Existing workspace

The Free Edition workspace owned by `databricks@pitgun.com` already contains
useful legacy experiments:

- `workspace.default.pitgun_events`, with historical telemetry events;
- `workspace.default.pitgun_runs`, with historical simulation summaries;
- `workspace.default.pitgun_physics_sweep_meta`, with early sweep metadata;
- an economy resource-sweep notebook and Python module;
- an old Git repository linked to the former TrackEagle physics work.

These assets remain untouched during V1. They are historical inputs, not the
target architecture. No existing table, notebook, dashboard, or repository may
be deleted or renamed until a separately reviewed retirement issue inventories
its lineage and confirms that it is no longer useful.

## Boundaries

### Pitgun owns

- simulation and policy contracts;
- the Rust Racing Solver and Simulator;
- deterministic identifiers, canonicalization, seeds, and run evidence;
- the parameter space and acceptance metrics for a campaign;
- selection and publication rules for opponent policies;
- the immutable policy artifact consumed by the Racing Catalog.

### Databricks owns

- offline campaign orchestration;
- parallel execution of independent configurations;
- governed Delta tables for runs, metrics, candidates, and policy releases;
- experiment tracking and comparison;
- lineage between campaign data and selected outputs;
- exploratory SQL, notebooks, and visualizations.

### Databricks does not own

- a Python rewrite of Pitgun physics;
- live player requests or leaderboard availability;
- Authority signing material or Verifier decisions;
- direct writes to the game database;
- hidden online adaptation to an individual player;
- the final decision that an artifact is safe to publish.

## Execution boundary

The current `pitgun demo racing` command executes one built-in scenario and is
not a general batch interface. V1 therefore requires a machine-readable runner
that:

1. accepts a resolved, versioned Racing scenario;
2. accepts an explicit seed and bounded parameter overrides;
3. executes the existing Rust workload through `pitgun-runtime`;
4. emits one canonical compact result containing identifiers and calibration
   metrics;
5. can optionally persist a complete Run Bundle for selected audit samples;
6. returns stable failure codes for invalid or incompatible inputs.

The runner must be usable locally before Databricks integration begins. Its
contract must remain useful to another orchestrator, such as a workstation,
Mac mini, CI runner, or future compute worker.

The Databricks adapter may package or invoke this runner from Python, but Python
must never duplicate the Solver or Simulator equations. The exact serverless
packaging mechanism is validated by a technical spike before the production job
is declared stable.

## Governed data model

V1 uses the existing managed `workspace` catalog and introduces two explicit
schemas:

```text
workspace
├── default                 legacy assets; unchanged during V1
├── pitgun_calibration      campaigns, runs, metrics, candidates
└── pitgun_policies         approved release metadata
```

The minimum Delta tables are:

| Table | Grain | Purpose |
|---|---|---|
| `pitgun_calibration.campaigns` | one campaign | Question, parameter space, versions, status, and timestamps |
| `pitgun_calibration.runs` | one configuration and seed | Canonical input identity, output summary, duration, and status |
| `pitgun_calibration.metrics` | one run and metric | Comparable pace, consistency, robustness, and strategy measurements |
| `pitgun_calibration.candidates` | one candidate and policy band | Selection scores, constraints, rank, and decision state |
| `pitgun_policies.releases` | one published policy version | Source campaign, artifact digest, catalog target, and approval metadata |

Every run row binds at least:

- `campaign_id`;
- a content-derived `configuration_id`;
- deterministic `run_id` when execution succeeds;
- scenario, model, and catalog identities and digests;
- source Git revision and runner version;
- explicit seed;
- circuit, era, setup, and strategy parameters;
- execution status and failure reason;
- compute duration and selected output metrics.

Raw per-frame telemetry is not written for every configuration. Compact outputs
are the default; full Run Bundles and telemetry are retained only for a bounded
sample or selected candidates. This avoids recreating the storage problem that
made continuous QuestDB ingestion unsuitable for the game.

## Experiment tracking

MLflow Experiments track campaign-level intent and comparison:

- input parameter-space version;
- simulator, model, and catalog versions;
- number of planned, successful, and rejected runs;
- aggregate pace, consistency, robustness, and diversity metrics;
- Delta table versions used by selection;
- plots and the candidate-policy artifact.

V1 does not use MLflow Model Registry. A deterministic opponent policy is a
versioned data artifact, not a trained machine-learning model. This boundary can
change only if a later Pitgun feature produces an actual model with an explicit
training and evaluation lifecycle.

## Job graph

The Databricks job is represented as code and follows this dependency graph:

```text
validate campaign
        ↓
materialize configurations
        ↓
execute deterministic runs
        ↓
derive comparable metrics
        ↓
select constrained candidates
        ↓
render report and policy proposal
```

Publication to `catalog.pitgun.io` is not automatic in V1. A human reviews the
candidate report and promotes an immutable artifact through the existing catalog
publication workflow.

## Policy properties

A published Racing opponent policy must:

- be immutable and content-addressed;
- declare its schema and semantic version;
- bind its source campaign and catalog digest;
- define bounded profiles by circuit, era, and difficulty band;
- produce identical opponent inputs for the same policy, contract, and seed;
- avoid individual player identifiers or copied player setups;
- contain no post-result rubber-banding rule;
- remain optional so an older compatible catalog can continue to load.

The game may later use aggregated verified player outcomes to recalibrate
difficulty offline, but such data is outside V1 and requires a separate privacy
and product decision.

## Free Edition constraints

The first workspace is Databricks Free Edition. V1 therefore assumes serverless
compute, finite daily quotas, no availability guarantee, and synthetic Pitgun
data only. Jobs must be restartable and idempotent; quota exhaustion delays a
campaign rather than corrupting it.

The serverless SQL warehouse stays stopped when it is not needed. V1 records
enough timing and usage information to explain the cost boundary honestly even
when the direct monetary cost is zero.

## Acceptance criteria

Databricks Calibration V1 is complete when:

- [ ] one resolved scenario executes locally through the machine-readable runner;
- [ ] the same scenario and seed produce the same compact result across repeated runs;
- [ ] a versioned Databricks bundle deploys the schemas, experiment, and job;
- [ ] one campaign executes multiple configurations and seeds on serverless compute;
- [ ] every successful row is traceable to code, scenario, catalog, and run identities;
- [ ] invalid configurations are recorded explicitly rather than silently dropped;
- [ ] MLflow exposes the campaign parameters, aggregate metrics, and artifacts;
- [ ] candidate selection yields at least three materially different valid profiles;
- [ ] a reviewed policy artifact is content-addressed and published through the Racing Catalog;
- [ ] the game loads that policy without recompiling its Rust/WASM simulator;
- [ ] one deterministic fixture proves that policy, contract, and seed reproduce the same opponents;
- [ ] the public case study reports the real campaign size, result, and limitations.

## Non-goals

- covering every Racing circuit and era in the first campaign;
- real-time or private-championship matchmaking;
- LLM-controlled opponents;
- online learning from individual players;
- replacing Hosted Verification;
- building a generic hosted Databricks service for third-party models;
- deleting or migrating legacy workspace assets as part of the initial delivery.

## Delivery order

1. Define and implement the generic machine-readable run boundary.
2. Bootstrap the Databricks bundle and governed schemas.
3. Validate serverless packaging of the Rust runner.
4. Execute and analyze the reference Racing campaign.
5. Select and publish a versioned policy proposal.
6. Integrate the policy into the game and deterministic fixtures.
7. Publish the reproducible technical case study.

Private Championships begin only after this vertical slice is complete or
explicitly paused with its reusable outputs documented.

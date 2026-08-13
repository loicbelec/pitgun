# Racing Batch Runner V1

Status: implemented as the first machine-readable execution boundary for
offline experiment campaigns, including Databricks calibration workloads.

## Purpose

The batch runner executes one fully resolved Racing scenario with one explicit
seed and emits a compact canonical JSON result:

```text
Resolved scenario + seed -> deterministic execution -> compact result
```

It is a local process boundary. It does not require HTTP, an account, the
Pitgun VPS, Databricks, or any database. Databricks is one possible
orchestrator: it can materialize scenarios, invoke the binary in parallel, and
store the resulting JSON rows.

The human-oriented `pitgun demo racing` command remains the interactive proof
of the complete persist/replay/verify loop. The batch runner is the automation
surface for parameter sweeps.

## Command

```bash
pitgun run racing \
  --scenario apps/pitgun-cli/scenarios/racing-demo-v1.json \
  --seed 42
```

Catalog-backed models such as Racing V2 must also receive the exact immutable
release directory. The CLI reads reviewed local bytes only; it never resolves
HTTP or a mutable `latest` alias:

```bash
pitgun run racing \
  --scenario /path/to/racing-v2-scenario.json \
  --catalog-release catalogs/racing/v1.2.0 \
  --seed 42
```

The release manifest must select the scenario's exact model identity and
Simulation Pack. Missing, malformed or incompatible releases fail before
simulation.

The canonical compact result is written to stdout. It can instead be written
to a file:

```bash
pitgun run racing \
  --scenario scenario.json \
  --seed 42 \
  --result result.json
```

A complete immutable Run Bundle V1 can also be retained for selected runs:

```bash
pitgun run racing \
  --scenario scenario.json \
  --seed 42 \
  --result result.json \
  --bundle run-bundle
```

| Argument | Meaning |
|---|---|
| `--scenario <PATH>` | Required resolved Racing scenario JSON document. |
| `--seed <U64>` | Required deterministic root seed. |
| `--catalog-release <DIR>` | Immutable catalog directory required by catalog-backed model versions. |
| `--result <PATH>` | Optional compact-result destination. If omitted, stdout is used. |
| `--bundle <PATH>` | Optional exact destination for a complete Run Bundle V1. |

Normal success is quiet on stderr. Supplying `--result` leaves stdout empty,
which makes the command straightforward to use in task runners.

## Input Contract

The input document uses:

```json
{
  "schema_version": "pitgun.racing-resolved-scenario/v1"
}
```

The complete executable fixture is
[`racing-demo-v1.json`](../apps/pitgun-cli/scenarios/racing-demo-v1.json).
It contains the scenario identity, model and data-pack identities, circuit,
vehicle, setup, strategy, opponents, telemetry definition, and derived-metric
configuration required for execution.

The runner intentionally accepts no individual physics or setup overrides.
An experiment planner must materialize each tested configuration as a complete
resolved scenario before invoking Pitgun. Therefore:

- every executed value is explicit and inspectable;
- `scenario_digest` identifies the canonical resolved document;
- `configuration_id` identifies the canonical model input;
- accidental mixtures of defaults and command-line overrides cannot create an
ambiguous experiment row.

The [`racing-batch-v1`](../apps/pitgun-cli/scenarios/racing-batch-v1)
fixture provides three completely materialized setup configurations. Its
integration test executes each configuration twice, proving distinct
configuration/run identities and byte-identical repetition without any Python
physics implementation.

Catalog discovery and transport remain separate concerns. A caller resolves a
specific immutable release first, then passes both its directory and the
domain-owned scenario to this command. The generic framework does not perform
catalog discovery or require HTTP.

## Success Result

The output document uses `pitgun.batch-run-result/v1`. It is encoded as
canonical JSON followed by one newline. For the same binary, scenario bytes
after canonicalization, and seed, repeated executions produce byte-identical
stdout.

The document contains:

| Field | Meaning |
|---|---|
| `runtime` | CLI implementation and version that executed the run. |
| `scenario` | Stable scenario identity and version. |
| `scenario_digest` | Digest of the canonical resolved scenario. |
| `model`, `data_pack` | Exact executable artifact identities. |
| `seed` | Deterministic root seed. |
| `configuration_id` | Digest of the resolved simulator input; the primary sweep configuration key. |
| `run_id` | Deterministic identity of the complete run contract. |
| `contract_digest` | Digest of the canonical deterministic run contract. |
| `output_digest` | Digest of the canonical simulator output. |
| `telemetry_summary_digest` | Digest of the telemetry evidence summary. |
| `metrics_digest` | Digest of the derived metrics document. |
| `summary` | Query-friendly race outcome, lap data, telemetry counts, derived metrics, and optional setup-response diagnostics. |

The compact result includes lap times and aggregate telemetry evidence, but not
the full telemetry batches. This keeps large sweeps economical. Use `--bundle`
for runs whose complete telemetry and replay artifacts must be preserved.

Operational values such as elapsed wall-clock time, host name, Databricks task
identifier, and output path are deliberately absent from the deterministic
result. An orchestrator may store them in a separate execution ledger.

### Setup-response diagnostics

When a player solution is present, `summary.setup_response` contains
`pitgun.racing-setup-response/v1`. It is derived by the canonical Racing Solver
from the physical samples and the applied vehicle, rather than reconstructed by
the data platform.

| Group | Fields and units |
|---|---|
| Definitions | Curvature and longitudinal-acceleration thresholds plus the near-maximum-RPM ratio used by this diagnostic version. |
| Circuit | length, straight/corner distance and elevation in metres; curvature in rad/m or integrated radians. |
| Time | observed, straight, corner, acceleration, braking, steady-speed, and near-maximum-RPM duration in seconds. |
| Speed | mean straight and corner speed in km/h. |
| Powertrain | shift count, maximum gear, maximum observed RPM, and maximum-RPM utilization ratio. |
| Aerodynamics | drag work in kJ; mean and maximum downforce in newtons. |

The straight/corner classifier uses the Solver's documented curvature boundary.
Diagnostics are explanatory and additive: they do not alter the run contract,
physics, telemetry, or lap time.

## Structured Failures

Failures are emitted as one `pitgun.batch-run-error/v1` JSON document on stderr;
stdout remains empty. The document contains `phase`, stable `code`, and a human
readable `message`.

Example:

```json
{"code":"scenario_unreadable","message":"cannot read scenario.json: ...","phase":"input","schema_version":"pitgun.batch-run-error/v1"}
```

| Exit code | Phases | Meaning |
|---:|---|---|
| `0` | — | Successful deterministic execution. |
| `2` | CLI | Invalid command grammar or option value, reported by the CLI parser. |
| `10` | `input`, `contract` | Scenario cannot be read, decoded, validated, or contracted. |
| `20` | `simulation` | Deterministic execution or metric processing failed. |
| `30` | `bundle`, `output` | Bundle persistence or compact-result encoding/writing failed. |

Callers must use the exit code and structured fields rather than parse the
human-readable message.

## Databricks Mapping

A calibration task can invoke the binary once per `(configuration_id, seed)`
pair and append the decoded result to a Delta table. `run_id` is the natural
idempotency key for a result, while `configuration_id` groups repetitions of
the same configuration across seeds.

The recommended campaign split is:

- keep compact results for every run;
- retain full bundles only for reference, regression, anomalous, or selected
  best-performing runs;
- keep orchestration metadata in a separate execution ledger;
- derive and publish a reviewed policy artifact rather than mutating the game
  directly from exploratory results.

See [Databricks Calibration V1](DATABRICKS_CALIBRATION_V1.md) for the complete
control-plane and data-plane architecture.

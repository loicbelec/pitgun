# Racing Demo CLI Contract V1

Status: incremental implementation tracked by
[#49](https://github.com/loicbelec/pitgun/issues/49). The simulation and typed
telemetry and run-bundle persistence phases are available; metrics, replay, and
verification remain planned.

## Purpose

The Racing demo is Pitgun's shortest complete proof:

**Scenario → Simulate → Observe → Persist → Replay → Verify**

It must run locally without an account, network service, VPS, container, or
external database. This document fixes the user-facing behavior needed by the
implementation tickets. Versioned schemas remain defined by their own contract
documents.

## Distribution Identity

The Cargo package remains named `pitgun-cli`. It exposes a binary named
`pitgun`, so the eventual installation and invocation are intentionally
different:

```text
cargo install pitgun-cli
pitgun demo racing --seed 42
```

Publishing the package to crates.io is not required by V1. Workspace execution
and prebuilt release binaries may provide the command first.

## Command Grammar

```text
pitgun demo racing [--seed <U64>] [--output <PATH>]
```

The current persisted-simulation increment accepts both `--seed` and `--output`.

| Argument | Meaning |
|---|---|
| `--seed <U64>` | Unsigned decimal seed recorded in the deterministic run contract. Defaults to `42`. |
| `--output <PATH>` | Exact destination directory for the run bundle. It must not contain an unrelated or conflicting bundle. |

The documented quickstart uses an explicit `--seed 42` even though `42` is the
default. This makes the source of randomness visible to a new user.

V1 deliberately has no remote endpoint, custom scenario, registry, database,
quiet mode, machine-report mode, or destructive `--force` option. Those options
must be justified independently rather than inferred from this contract.

## Execution Flow

One successful invocation performs every phase in the same process:

1. Load the built-in, versioned Racing reference scenario.
2. Build and validate its deterministic run contract with the requested seed.
3. Execute the Racing model and collect typed telemetry.
4. Calculate the configured derived telemetry metric.
5. Construct and validate a complete run bundle in a staging directory.
6. Commit the bundle to its destination.
7. Reload the committed bundle, replay its telemetry, and recalculate evidence.
8. Verify all required identities and print the final report.

`VERIFIED` describes the complete committed bundle. It must never be printed
after simulation alone or before replay and verification have succeeded.

## Run Directory

When `--output` is omitted, Pitgun writes below `./pitgun-runs/`. The leaf
directory is the complete file-safe run identity:

```text
pitgun-runs/sha256-<64 lowercase hexadecimal characters>/
```

The bundle manifest retains the canonical `sha256:<hex>` spelling. Replacing the
colon with a hyphen is only a cross-platform filesystem convention and does not
change the logical `run_id`.

When `--output <PATH>` is supplied, that path is the exact bundle directory; no
additional run-id directory is appended. Relative paths are resolved from the
current working directory. Pitgun creates missing parent directories, but the
destination itself must either be absent or satisfy the verified-reuse rule
below; a regular file at that path is always an error.

### V1 file names

The bundle contract in #66 owns the schemas and canonical encoding. The CLI
reserves these user-visible names:

```text
manifest.json
scenario.json
contract.json
output.json
telemetry.jsonl
telemetry-summary.json
metrics.json
receipt.json
```

`manifest.json` is committed last and identifies the bundle as complete. Paths
stored inside it are relative so the directory can be moved without changing
logical evidence.

The current #66 increment writes every listed file except `metrics.json`, which
is added by #69. Its optional manifest slot and user-visible name are already
reserved. See [Deterministic Run Bundle V1](RUN_BUNDLE_V1.md) for schema and
validation details.

## Collision and Commit Behavior

Run bundles are immutable evidence. V1 does not overwrite them.

- If the destination does not exist, Pitgun writes to a temporary sibling and
  commits it only after local validation succeeds.
- If a complete bundle for the same `run_id` already exists, Pitgun executes the
  requested run, compares the newly calculated evidence, verifies the existing
  bundle, leaves it unchanged, and reports it as reused.
- If the existing path is incomplete, invalid, belongs to another run, or
  differs from the newly calculated evidence, Pitgun fails without modifying it.
- A failed invocation must not leave a path that can be mistaken for a complete
  bundle. Temporary cleanup is best-effort, but only a valid manifest marks a
  bundle as committed.

This behavior makes the documented command safely repeatable while preventing a
corrupt or unexpected artifact from being silently replaced.

## Human Report

On success, stdout contains a concise report shaped like this:

```text
Pitgun Racing deterministic demo

scenario    pitgun.racing-demo/v1
seed        42
run_id      sha256:<hex>
telemetry   <frame-count> frames in <batch-count> batches
metric      <metric-id> = <value> <unit>
bundle      <path> (created|reused)
replay      OK
verification VERIFIED

VERIFIED sha256:<hex>
```

The labels, spacing, and explanatory rows are presentation and may improve
without a schema change. The final `VERIFIED <run_id>` line, process exit code,
and versioned bundle files are the V1 automation boundary. Consumers needing
structured details must read the manifest rather than parse the human table.

Normal success is quiet on stderr. Diagnostics, optional progress logging, and
errors go to stderr so stdout remains suitable for the final report.

## Exit Codes

| Code | Meaning |
|---:|---|
| `0` | The committed or reused bundle completed replay and verification. |
| `1` | Unexpected internal failure not covered by a more specific category. |
| `2` | Command-line usage error, including an invalid seed or missing option value. |
| `10` | Built-in scenario or deterministic contract is invalid or incompatible. |
| `20` | Simulation or derived telemetry processing failed. |
| `30` | Bundle staging, persistence, collision, or filesystem validation failed. |
| `40` | The committed bundle could not be loaded or replayed. |
| `50` | Replay completed, but deterministic verification failed. |

Errors include the failed phase, the relevant path or artifact when safe, and an
actionable reason. They must not print `VERIFIED`. Internal debug details and
backtraces are opt-in and must not redefine the documented exit category.

## Stable and Evolvable Surfaces

V1 treats the following as machine contracts:

- command and option names documented above;
- exit-code categories;
- the exact final `VERIFIED <run_id>` success line;
- reserved bundle file names;
- versioned schemas and canonical evidence referenced by the bundle.

The following remain presentation details:

- whitespace, alignment, colors, and row ordering in the human report;
- progress messages and logging;
- temporary directory names;
- elapsed wall-clock duration and other non-canonical diagnostics.

Any future output that may vary by operating system, build, or wall clock stays
outside the logical run evidence.

## Implementation Boundaries

- #65 implements command parsing, the reference scenario, simulation, and typed
  telemetry collection.
- #66 defines the run-bundle V1 schemas, canonical files, and persistence logic.
- #69 selects and implements the derived telemetry metric.
- #67 implements reload, replay, verification, and the final report.
- #70 protects the public flow and exit behavior in hermetic CI.
- #68 packages the binary and validates the under-five-minute quickstart.

# Deterministic Run Bundle V1

## Purpose

A run bundle is Pitgun's portable record of one deterministic computation. It
keeps the logical inputs, domain output, and telemetry together so a later tool
can inspect, replay, and verify them without the original process or machine.

The V1 directory is committed only after every file has been written and
validated. `manifest.json` is written last and is the completion marker.

## Layout

```text
<bundle>/
├── manifest.json
├── scenario.json
├── contract.json
├── output.json
├── telemetry.jsonl
├── telemetry-summary.json
└── receipt.json
```

`metrics.json` is a reserved optional canonical artifact. It becomes present
when the derived-metric increment tracked by #69 is implemented.

All paths in `manifest.json` are fixed relative file names. Moving or copying
the complete directory therefore does not change any identity.

## Schema identities

| Artifact | V1 schema identity |
|---|---|
| Manifest | `pitgun.run-bundle-manifest/v1` |
| Built-in Racing scenario | `pitgun.racing-demo-scenario/v1` |
| Deterministic contract | `pitgun.deterministic-run-contract/v1` |
| Racing output | `pitgun.racing-output/v1` |
| Each telemetry JSONL record | `pitgun.telemetry-record/v1` |
| Telemetry summary | `pitgun.telemetry-summary/v1` |
| Receipt wrapper | `pitgun.run-bundle-receipt/v1` |

JSON artifacts use RFC 8785 canonical encoding. `telemetry.jsonl` contains one
RFC 8785 value followed by `LF` per ordered frame. Every record includes a
zero-based ordinal; the file must end with `LF` when it is non-empty.

## Logical evidence and execution evidence

The manifest separates two kinds of proof:

- `canonical_artifacts` identifies the scenario, contract, output, telemetry,
  summary, and eventually metrics. These digests must be identical when the
  same logical run is repeated.
- `execution_artifacts` identifies `receipt.json`. The receipt records a UUIDv7,
  CLI version, compilation target, and digest of the concrete executable. It may
  differ between genuine execution attempts without changing `run_id`.

The receipt is nevertheless content-addressed and bound to the contract,
canonical output, and telemetry summary. Runtime identity is evidence about how
the result was produced; it is not an input to the logical simulation.

## Persistence and collisions

For a new destination, the CLI:

1. creates a temporary sibling directory;
2. writes every artifact except the manifest;
3. constructs the manifest from the exact stored-byte digests;
4. writes `manifest.json` last;
5. reloads and validates the complete staged bundle;
6. atomically renames the directory to its final path.

An existing bundle is never overwritten. The CLI validates it and reuses it
only when its `run_id` and all newly calculated canonical artifact references
match. An incomplete, corrupted, or conflicting destination fails with exit
code `30` and remains untouched.

Validation covers the fixed relative layout, canonical JSON encoding, every
referenced digest, the contract-derived `run_id`, sequential telemetry records,
the summary frame count, and the receipt bindings.

## Current boundary

Bundle persistence proves **Scenario → Simulate → Observe → Persist** and still
reports `SIMULATED`. Loading the committed telemetry as a replay source,
recalculating evidence, and printing `VERIFIED` belong to #67.

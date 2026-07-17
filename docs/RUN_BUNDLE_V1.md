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
├── metrics.json
└── receipt.json
```

`metrics.json` contains deterministic values derived from the recorded typed
telemetry and their complete versioned processor configuration.

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
| Derived metrics | `pitgun.derived-metrics/v1` |
| Receipt wrapper | `pitgun.run-bundle-receipt/v1` |

JSON artifacts use RFC 8785 canonical encoding. `telemetry.jsonl` contains one
RFC 8785 value followed by `LF` per ordered frame. Every record includes a
zero-based global `ordinal` and zero-based `batch_ordinal`; the file must end
with `LF` when it is non-empty. Batch ordinals are contiguous and preserve the
transport boundaries required to reproduce the telemetry summary exactly.

## Logical evidence and execution evidence

The manifest separates two kinds of proof:

- `canonical_artifacts` identifies the scenario, contract, output, telemetry,
  summary, and metrics. These digests must be identical when the same logical
  run is repeated.
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
the summary frame count, the metrics schema, and the receipt bindings.

## Derived metric V1

The Racing reference workload configures one metric:

```text
id            racing.observed-maximum-speed
processor     pitgun.telemetry-aggregate/v1
parameter     5005
statistic     maximum
unit          km/h
```

This is the maximum speed **observed in the emitted 5 Hz telemetry**, not a
value copied from the simulator result. The domain-neutral processor considers
finite numeric samples with `good` or `degraded` quality and records the exact
sample count alongside the result. Racing supplies only the meaning of
parameter `5005` and its display unit.

This distinction demonstrates the Observe stage: changing a recorded speed
sample changes `metrics.json` even if the domain result is left untouched. The
same aggregate can later calculate a maximum temperature, load, voltage, or any
other typed scalar without adding domain concepts to `pitgun-core`.

## Replay and verification

```bash
pitgun replay <BUNDLE>
```

The reader operates exclusively on the committed directory. It:

1. loads canonical schemas and the fixed portable layout;
2. replays telemetry records by global and batch ordinal;
3. verifies every referenced stored-byte digest;
4. derives `run_id` from `contract.json` and checks scenario/model/input bindings;
5. verifies the execution receipt's output and telemetry-summary bindings;
6. recalculates the telemetry summary from all replayed frames;
7. executes each declared metric processor against those frames;
8. prints `VERIFIED <run-id>` only when every comparison succeeds.

Missing, malformed, non-canonical, or non-replayable evidence exits with code
`40`. A loaded bundle whose declared and recalculated evidence differs exits
with code `50`. Diagnostics identify the first failing artifact or invariant.

This completes **Scenario → Simulate → Observe → Persist → Replay → Verify**.

### Trust model

Verification establishes deterministic self-consistency, not independently
trusted provenance. A coordinated adversarial rewrite of all artifacts and
digests requires a separate signature or external trusted root to detect. The
V1 receipt records concrete runtime identity but is not itself a signature.

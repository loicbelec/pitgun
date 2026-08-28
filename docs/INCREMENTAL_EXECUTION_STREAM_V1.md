# Incremental Execution Stream V1

Status: domain-neutral contract implemented for issue #397. Racing is the
first planned producer under #398; this contract does not change an existing
workload or staging deployment by itself.

## Purpose

The current Racing browser boundary returns only after a complete session has
been calculated. Pitgun needs to expose useful deterministic progress while a
heavier model is still executing, without making render cadence, callbacks, or
network timing part of the simulation.

Incremental Execution Stream V1 separates those concerns:

```text
deterministic workload -> bounded ordered batches -> consumer buffer -> UI
```

The workload produces records as fast as its deterministic boundaries allow.
A consumer may display them immediately, buffer them, replay them slowly, or
persist them. None of those choices may change the record bytes or final
result.

## Descriptor

`IncrementalExecutionStreamDescriptorV1` is immutable for one concrete stream:

- `schema_version`: `pitgun.incremental-execution-stream/v1`;
- `execution_id`: the canonical UUIDv7 of the concrete attempt;
- `model`: the exact resolved, content-addressed model identity;
- `clock`: the rational fixed-step logical clock used by record ticks.

The `execution_id` exists before the result is complete. A workload whose final
`run_id` depends on accepted decisions must include that final identity in its
domain-owned completion payload rather than pretending it was known at stream
start.

## Records and batches

Each record contains:

- a zero-based `sequence` in one contiguous total order;
- a non-decreasing integer `logical_tick` interpreted through the descriptor;
- either a domain-owned `progress` payload or the unique terminal `complete`
  payload.

Records are transported in non-empty batches of at most 256 items. A batch may
start at any sequence when decoded independently, but
`IncrementalExecutionStreamCursorV1` requires the first batch to start at zero
and every later batch to continue exactly where the previous one ended.
Sequences and ticks remain within the exact I-JSON integer range.

```json
{
  "schema_version": "pitgun.incremental-execution-stream/v1",
  "records": [
    {
      "sequence": 0,
      "logical_tick": 0,
      "event": {
        "kind": "progress",
        "payload": { "phase": "started" }
      }
    }
  ]
}
```

The contract deliberately does not define Racing positions, telemetry
channels, pit-stop events, or results. Those are typed workload payloads and do
not enter the generic framework crate.

## Completion and parity

Exactly one `complete` record terminates a valid stream. It must be the last
record of its batch. Any subsequent record or batch fails closed. The workload
completion payload carries its existing canonical result and evidence inputs.

Collecting all records from the future incremental Racing producer must yield
the same final output as its current monolithic entry point. Incremental
execution is an observability and interaction boundary, not a second physics
implementation.

## Backpressure and cancellation

The fixed batch bound limits one hand-off, not the complete session. A pull
adapter may stop requesting batches while its consumer catches up; a push
adapter must use a bounded queue. Dropping, reordering, or silently merging
records is forbidden.

Cancellation is observable only between complete records or batches. A
cancelled execution has no terminal completion and therefore cannot be treated
as a completed or verified run. V1 does not prescribe transport-specific
cancel, resume, callback, iterator, or handle APIs.

## Compatibility and trust

- existing deterministic run, telemetry, Racing output, and Hosted
  Verification schemas are unchanged;
- the contract contains no HTTP, WebSocket, WASM, browser, or UI type;
- presentation timing is explicitly non-authoritative;
- Hosted Verification continues to verify the final evidence rather than
  trusting progress shown by a browser;
- adding a producer is a separate change and requires native/WASM parity before
  staging can select it.

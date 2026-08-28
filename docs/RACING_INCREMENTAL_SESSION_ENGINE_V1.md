# Racing incremental session engine V1

The Racing V3 workload can now be executed as a deterministic pull stream.
This is a computation boundary, not a playback or network protocol.

## Boundary

`ResolvedSimulationSessionV3` owns the physical state for one competitor. One
call to `advance` computes one complete lap and yields:

- the lap-local physical solution;
- cumulative time and distance;
- fuel, tire wear, tire temperature and engine temperature after the lap;
- terminal speed, selected gear and remaining shift interruption;
- the active resolved driver control and correction workload;
- whether pit service completed at that boundary.

The future-backed implementation suspends the existing numerical kernel at the
lap boundary. It does not recompute prefixes and does not calculate the whole
race before the first progress item.

`IncrementalRacingSession` owns one Solver session per competitor. It advances
the complete field at the same logical lap boundary and emits bounded records:

1. a grid snapshot containing distance, lap, position, time and status;
2. player telemetry in envelopes of at most 64 frames;
3. ordered lap, pit-stop and driver-instruction events;
4. one terminal record containing the existing `RaceOutput`.

No wall-clock time, render frame, callback, HTTP or WebSocket concept enters
the Solver or Simulator.

## Compatibility

The existing V3 entry points remain synchronous. They collect
`IncrementalRacingSession` until its terminal record and return the unchanged
`RaceOutput`.

Tests prove that:

- ten competitors produce progress before completion;
- two identical inputs produce identical canonical record bytes;
- every telemetry record contains at most 64 frames;
- the incremental terminal output equals the compatibility collector output;
- the Solver's incremental terminal result equals its previous monolithic
  result while carrying state across lap boundaries.

## Published parity vector

`crates/pitgun-racing-simulator/tests/fixtures/incremental_racing_stream_parity_v1.json`
is the reusable V1 conformance vector for the native engine and the future
WASM pull adapter. Its fixed two-lap scenario contains ten competitors and
publishes:

- the canonical stream descriptor and exact Model V3 identity;
- the sequence range, record count, item count and conformance digest of every
  ordered batch;
- representative grid, telemetry, lap-event and completion record vectors;
- portable whole-stream and terminal-shape digests.

The descriptor uses one nominal fixed logical second per completed-lap tick.
This clock orders pull boundaries only; simulated lap and telemetry time remain
in the Racing payload and are not replaced by wall-clock time.

The test deliberately separates two guarantees. Within one runtime, two native
executions must reproduce the exact canonical stream bytes and the terminal
bytes must equal the synchronous compatibility collector. Across native and
WASM math libraries, the published conformance projection hashes portable
ordering and envelope semantics without pretending that the last bits of V3
floating-point telemetry are already platform-independent.

Readable artifacts are compared in order before the aggregate projection
digest, so an intentional change identifies its first affected batch or
record. The test also rejects reordered, missing, duplicated and
post-completion records. Do not refresh this fixture only to make a failure
pass: an intentional observable change must explain its determinism and
compatibility impact first.

## Deliberate limits

- V1 yields at completed-lap boundaries. Segment-level or fixed-duration
  windows may be introduced later without changing the transport-neutral
  stream contract.
- Visual cadence and browser pull bindings belong to follow-up work.
- Live driver decisions are not accepted during an execution yet; the current
  instruction timeline remains predeclared and deterministic.
- Retained state is bounded by the validated race length, track sample count,
  ten-competitor field and 64-frame transport envelopes.

# Racing WASM pull session V1

The domain-owned `pitgun-racing-simulator` crate exposes the governed
incremental Racing engine to JavaScript as a synchronous pull API. The
transitional `pitgun-solver` crate forwards the same functions for downstream
compatibility. The API lets a browser yield between deterministic lap
boundaries without moving rendering, timers, callbacks, or networking into the
simulation core.

## Lifecycle

1. `start_authorized_dynamic_racing_session_with_catalog_json` validates the
   existing Authority request and immutable Racing catalog, then returns an
   opaque process-local handle without solving the first lap.
2. `pull_authorized_dynamic_racing_session_json` advances all competitors to
   the next common lap boundary and returns one ordered stream batch.
3. Once a pull reports `complete: true`,
   `complete_authorized_dynamic_racing_session_json` consumes the handle and
   returns the same application result and Verifier evidence as the monolithic
   compatibility collector.
4. `release_authorized_dynamic_racing_session_json` discards an active or
   completed handle when a browser abandons the session.

Handles are local resource identifiers only. They are excluded from the signed
contract, stream records, receipts, run identity, and verifier evidence.

## Deterministic boundary

The host chooses when to call `pull`, but not how much model time advances. One
successful pull crosses exactly one governed session boundary. Waiting,
rendering, interleaving another handle, or releasing an unrelated handle cannot
change emitted stream bytes or the terminal run identity.

The final pull includes the native completion record. The explicit completion
call then exposes the complete application projection and destroys the handle.
Calling `pull` after terminal progress, calling `complete` too early, or reusing
a consumed/released handle fails closed.

## Compatibility proof

The native and Node/WASM golden test:

- reproduces the portable batch vectors published in
  `incremental_racing_stream_parity_v1.json`;
- executes identical sessions under different host interleavings and compares
  their exact bytes within each runtime;
- compares the terminal application result with the existing monolithic API;
- verifies early completion, post-completion pulls, and abandoned-handle
  cleanup.

The existing monolithic exported functions remain supported and now collect the
same authorized incremental domain session internally.

## Solved competitor trajectories

Authorized and local pull sessions now populate the optional `trajectory` field
on each grid competitor. Each point is `[elapsed_seconds, unwrapped_metres]`,
using that competitor's own cumulative clock and distance. A lap begins at the
previous lap's endpoint (the first lap begins at `[0, 0]`). A stationary endpoint
interval preserves pit-service time. Samples are copied from the solved lap;
no extra physics solve or browser-side pace estimate is involved.

This changes authorized **progress JSON bytes** by adding optional samples. The
checkpoint envelope, sequence, logical ticks, portable parity projections,
terminal `RaceOutput`, and verification evidence projections are unchanged.
Generic checkpoint-only incremental sessions remain available. Historical streams
without trajectories remain readable. Consumers must not treat progress samples
as part of the accepted result or publish future classification before playback.
A rebuilt WASM has its own artifact digest, as usual; it does not change the
model, catalog, input, output, or telemetry-summary identity.

The native/Node-WASM golden tests cover both authorized models, ten competitors,
first-lap origins, lap continuity, pit intervals, and exact output/evidence parity
against checkpoint-only execution. To export public dummy-authorized inputs for
checking a shipped browser module, run:

```sh
PITGUN_EXPORT_PLAYBACK_FIXTURES=/tmp/pitgun-playback-fixtures \
  cargo test -p pitgun-solver --test racing_golden authorized_trajectories
```

These fixtures contain deliberately invalid cryptographic signatures and must
never be submitted to Authority or Verifier. They exercise the browser's existing
shape/integrity validation; cryptographic acceptance stays at the trusted service.

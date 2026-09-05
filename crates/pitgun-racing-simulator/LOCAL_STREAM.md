# Local Racing pull API v1

The local power-unit thermal model can be advanced using four WASM exports:

- `start_local_racing_session_with_catalog_and_v3_power_unit_thermal_profile_json(input, catalog, profile)`
- `pull_local_racing_session_json(handle)`
- `complete_local_racing_session_json(handle)`
- `release_local_racing_session_json(handle)`

Start takes exactly the same request, immutable catalog bundle and exact profile
bytes as `run_race_with_catalog_and_v3_power_unit_thermal_profile_json`. It selects
the identical V9 component profile, validates inputs and initializes the existing
`IncrementalRacingSession`; it does not compute a lap. Start, pull and release use
the existing `pitgun.racing-session-stream-{start,pull,release}/v1` envelopes.
Handles belong to this local registry and must not be passed to authorized APIs.

Each pull advances all competitors to one deterministic lap boundary. Its batch
contains the canonical `pitgun.incremental-execution-stream/v1` records, including
player frames and timestamped grid snapshots. The final pull sets `complete` and
contains the canonical terminal record. Complete then consumes the handle and
returns `{schema_version: "pitgun.racing-local-session-completion/v1", handle,
result: RaceOutput}`. Early completion fails without consuming the handle.
Release discards active or completed state; subsequent operations fail. Call
release after cancellation or errors. The browser should yield between pulls.

These functions create no signed authorization or verification evidence. Browser
cadence, rendering and transport do not enter the simulation. Historical and
vehicle-bound thermal exports retain their existing synchronous behavior.

`local_stream_preserves_output_and_handle_lifecycle` compares full JSON outputs
for 1, 3 and 8 laps with ten competitors, checks intermediate batches, early
completion, consumed handles, cancellation and malformed input. The frontend
also compares the generated WASM with its previously shipped binary on four
session configurations, including a pit stop.

The shared incremental adapter places every lap's telemetry on the cumulative
session clock/distance, using the Solver's exact previous lap boundary (including
pit time). Frame sequences increase across laps. These offsets change only the
progress stream; the terminal `RaceOutput` remains identical to the synchronous
API. The same correction applies to authorized incremental sessions.

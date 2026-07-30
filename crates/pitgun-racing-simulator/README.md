# pitgun-racing-simulator

`pitgun-racing-simulator` owns deterministic Racing orchestration.

It resolves the Racing catalog and strategies, evolves complete races and
sessions, invokes `pitgun-racing-solver`, projects telemetry, constructs
canonical Racing evidence, and implements the statically linked Racing
workload for `pitgun-runtime`.

It does not own the physical equations, generic deterministic contracts,
generic execution machinery, hosted authority policy, or game persistence.

The `wasm` feature exposes the browser-facing JSON facade from this crate. The
existing `pitgun-solver` package forwards the same functions until the game
switches to the new package in a coordinated release.

The browser catalog facade deliberately preserves the circuit, driver, vehicle,
and tire exports consumed by the game. It combines the immutable Racing
Simulation Pack with its separate Presentation Pack.

The source release lives under `catalogs/racing/v1.0.0`. A generated Rust file
embeds the same Simulation Pack for native and WASM recovery, so hosted and
fallback resources cannot drift through two manually maintained file lists.

## Hosted verification evidence

The strict browser API is exposed as:

- `execute_authorized_race_json`;
- `execute_authorized_race_with_catalog_json`.

Both accept `RacingHostedExecutionRequestV1` and return a complete
`RacingVerificationSubmissionV1`: the unchanged Authority authorization,
canonical input, execution receipt, canonical Racing output and telemetry
summary. The second function executes against caller-fetched immutable catalog
bytes; the first uses the byte-identical embedded recovery release.

The browser loader must calculate `wasm_artifact_digest` over the exact
downloaded `.wasm` bytes before instantiation. A module cannot truthfully hash
its own final bytes from inside itself. The digest is recorded as runtime
provenance in the receipt; independent replay remains the verification trust
boundary.

The existing presentation-oriented race and session functions remain
unchanged during game migration.

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

The browser catalog facade deliberately keeps circuit display metadata and the
driver, vehicle, and tire list exports consumed by the game. These presentation
views are owned here rather than by the canonical cross-process Racing contract.

The embedded data pack remains temporarily stored under the transitional
`pitgun-simulator` directory until that crate is retired.

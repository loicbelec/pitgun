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

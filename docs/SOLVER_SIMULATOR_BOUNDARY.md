# Solver / Simulator Boundary

This document defines the intended boundary between `pitgun-solver` and
`pitgun-simulator`.

## Intent

- `pitgun-solver` is the deterministic compute kernel for lap and race
  simulation.
- `pitgun-simulator` is the data-pack, catalog, and runtime adapter around that
  kernel.

The goal is to keep one simulation math implementation while still exposing a
domain-friendly API for tools, WASM, and the game.

## `pitgun-solver` owns

- physics and simulation math
- tuning application
- tire / engine / aero / chassis compute behavior
- lap and race solution generation
- telemetry resampling derived from solver outputs
- deterministic behavior for the same resolved inputs

`pitgun-solver` should receive resolved structures, not repository-specific file
paths or JSON pack semantics.

## `pitgun-simulator` owns

- embedded simulator data pack
- loading and validating JSON resources
- catalog and listing APIs for assets
- mapping ids like `vehicle_id`, `track_id`, `driver_id` to resolved solver inputs
- WASM-friendly access to embedded simulator resources
- ergonomic runtime APIs such as `Simulator` and `run_simulation(...)`

`pitgun-simulator` may adapt requests and outputs, but it must not carry an
independent physics implementation.

## What should not live in either crate

- gateway analytics
- LLM-oriented summaries
- persistence concerns
- gameplay UI orchestration
- product-specific coaching logic

Those belong in services, apps, or higher-level product code.

## Practical rule

If a change answers “how does the vehicle behave?”, it probably belongs in
`pitgun-solver`.

If a change answers “how do we load, resolve, package, or expose simulator
resources?”, it probably belongs in `pitgun-simulator`.

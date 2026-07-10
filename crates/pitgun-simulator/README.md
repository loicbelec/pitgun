# pitgun-simulator

`pitgun-simulator` is the racing domain simulator for Pitgun.

It owns the lap-time simulation model, racing data pack, catalog resolution, and
runtime APIs used by the game and WASM builds. It is the first domain
application built on top of the generic Pitgun framework.

- Simulation defaults live in the embedded JSON data pack under `data/`.
- Native builds can layer an external pack on top of the embedded defaults with `DataRegistry::load_from_dir(...)`.
- WASM builds always use the embedded pack.

To add or change simulator data:

1. Add or update the JSON file in the matching category under [`data/`](/Users/loic/Code/pitgun/pitgun/crates/pitgun-simulator/data).
2. Keep `schema_version` at the supported version and use a stable `id`.
3. If the file changes a referenced object (`vehicle`, `driver`, etc.), make sure the referenced ids already exist in the pack.
4. Run `cargo test -p pitgun-simulator`.

## Boundary

`pitgun-simulator` should own racing concepts:

- vehicles, engines, tires, chassis, aero, drivers, tracks, pit stops
- lap-time and race simulation behavior
- hybrid energy state and deployment behavior
- racing telemetry generated from simulation outputs
- mapping racing ids such as `vehicle_id`, `track_id`, or `driver_id` to resolved model inputs

Generic framework crates should not need to know what these concepts mean. They
should ingest, validate, sign, route, and process them as payloads, samples,
metadata, registries, or policy-controlled parameters.

`pitgun-solver` currently still contains racing simulation code. The target
architecture is to move that code into this crate and reserve `pitgun-solver`,
if it remains, for generic deterministic compute and verification concerns.

## Public API

The intended public surface of `pitgun-simulator` is:

- `DataRegistry` and the provider types for loading and listing simulator assets
- config models such as `VehicleConfig`, `TrackConfig`, `EngineConfig`, and friends
- runtime entry points such as `run_simulation(...)`
- ergonomic façade types such as `Simulator`, `LapInput`, and `LapOutput`

The solver and frontend should pass explicit inputs. They should not redefine simulator defaults.

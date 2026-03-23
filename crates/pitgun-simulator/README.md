# pitgun-simulator

`pitgun-simulator` is the data pack and runtime adapter around `pitgun-solver`.

- The canonical simulator data pack lives first in `tooling/pitgun_simulator/data` and is mirrored here for embedding and WASM distribution.
- Embedded defaults live in the JSON data pack under [`data/`](/Users/loic/Code/pitgun/pitgun/crates/pitgun-simulator/data).
- Native builds can layer an external pack on top of the embedded defaults with `DataRegistry::load_from_dir(...)`.
- WASM builds always use the embedded pack.

To add or change simulator data:

1. Update the canonical JSON file under `tooling/pitgun_simulator/data`.
2. Run `./scripts/sync-simulator-data.sh` from the `framework/` repo root.
3. Keep `schema_version` at the supported version and use stable ids when the schema supports them.
4. If the file changes a referenced object (`vehicle`, `driver`, etc.), make sure the referenced ids already exist in the pack.
5. Run `cargo test -p pitgun-simulator`.

`pitgun-solver` remains the source of truth for simulation math. `pitgun-simulator` should not maintain a divergent physics implementation.

## Public API

The intended public surface of `pitgun-simulator` is:

- `DataRegistry` and the provider types for loading and listing simulator assets
- config models such as `VehicleConfig`, `TrackConfig`, `EngineConfig`, and friends
- runtime entry points such as `run_simulation(...)`
- ergonomic façade types such as `Simulator`, `LapInput`, and `LapOutput`

Lower-level solver math helpers belong in `pitgun-solver`, not here.

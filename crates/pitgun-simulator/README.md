# pitgun-simulator

`pitgun-simulator` is the simulation source of truth.

- Simulation defaults live in the embedded JSON data pack under [`data/`](/Users/loic/Code/pitgun/pitgun/crates/pitgun-simulator/data).
- Native builds can layer an external pack on top of the embedded defaults with `DataRegistry::load_from_dir(...)`.
- WASM builds always use the embedded pack.

To add or change simulator data:

1. Add or update the JSON file in the matching category under [`data/`](/Users/loic/Code/pitgun/pitgun/crates/pitgun-simulator/data).
2. Keep `schema_version` at the supported version and use a stable `id`.
3. If the file changes a referenced object (`vehicle`, `driver`, etc.), make sure the referenced ids already exist in the pack.
4. Run `cargo test -p pitgun-simulator`.

The solver and frontend should pass explicit inputs. They should not redefine simulator defaults.

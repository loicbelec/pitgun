# Racing Model V3 thermal runtime integration V1

Status: native Rust and browser/WASM integration candidate. No catalog or
hosted-verification promotion is authorized by this boundary.

## Runtime ownership

The Racing Simulator parses the exact reviewed thermal-family artifact and
selects one profile by `vehicle_id`. The solver receives only the resulting
`engine_thermal_resolution`; it does not know about vehicle families, catalog
selection, Databricks or release policy.

The four reviewed bindings are:

| Vehicle | Family |
|---|---|
| `classic_v8_1960` | `historical_v8` |
| `classic_v8_1970` | `historical_v8` |
| `modern_v6t` | `modern_v6t` |
| `f1_2026` | `f1_2026` |

Era-only selection is forbidden because `modern_v6t` and `f1_2026` both use
era 5 while retaining different validated coefficients. `default` and every
other unreviewed vehicle identifier fail closed.

## Content boundary

The parser accepts only candidate digest
`sha256:8aefd230da307e3439eef115fbfcd1117c8a8bbb1128c2c4b00138d6026f2f57`.
Changing whitespace or any value changes the exact resource digest and is
rejected before execution.

Each successful `RaceOutput` records:

- candidate id, version and digest;
- Model V3 id, version and digest;
- selected family and exact vehicle id;
- parameter-set id and validated profile reference.

The experimental 130 kg fuel reservoir is never read by this runtime. The
normal Model V3 default remains in effect until the separate fuel/energy
contract is reviewed.

## Portable APIs and evidence

- native Rust:
  `run_race_with_catalog_and_v3_thermal_family_profile`;
- browser/WASM JSON:
  `run_race_with_catalog_and_v3_thermal_family_profile_json`.

Portable tests execute all four bindings through both boundaries and compare
their canonical outputs byte for byte. They also cover an unknown vehicle and
mutated candidate bytes. The same test suite runs natively and under Node/WASM.

## Route to a game-scale test

1. Merge the native/WASM integration without changing a published workload.
2. Build a new Racing Catalog candidate that pins the Model V3 and thermal
   profile identities.
3. Add that exact model/catalog pair to Authority and Verifier.
4. Deploy the coordinated WASM and catalog candidate to game staging.
5. Run complete weekends for historical V8, modern V6T and F1 2026, submit
   verified scores and inspect thermal/cooling diagnostics.
6. Measure the resulting player decision surface before changing UI controls.

The fifth step is the first genuine game-scale test. Production remains
unchanged until those staging observations are reviewed.

# Racing Game Vehicle Contract V1

Status: reviewed cross-repository contract for issue #248. This contract repairs
vehicle selection for the public game without inventing unproven historical
physics or changing a historical Racing model identity.

## Decision

The public game may select only vehicle resources that exist in its pinned
Racing Catalog Simulation Pack. Eras 3 and 4 continue to use the governed
`classic_v8_1970` physical base until a distinct generation has reviewed
component data and parameter provenance.

This is deliberately more honest than restoring the old names:

- `classic_v6t_1980` was an exact component alias of `modern_v6t`;
- `classic_v10_1990` and `classic_v8_2000` differed mostly through unreviewed
  engine curves, while reusing the same chassis, tire, and effectively the same
  aerodynamic bytes;
- publishing those candidates would close a lookup gap while falsely implying
  three validated physical generations.

Game progression is not frozen by this choice. Era 3 and Era 4 upgrades still
change resolved development and setup inputs. What they do not do is silently
select a nonexistent vehicle or claim a new physical vehicle generation.

## Public-release mapping

The public release ends at Era 5. Vehicle selection remains conditional on the
upgrades actually owned by the player, so an era can legitimately retain an
earlier vehicle.

| Vehicle | Earliest era | Required upgrade | Catalog 1.3.0 components | Claim |
| --- | ---: | --- | --- | --- |
| `classic_v8_1960` | 1 | none | `v8_1960` / `none` / `default` / `medium` | initial mechanically dominated base |
| `classic_v8_1970` | 2 | `e2_v8_engine` | `v8_1970` / `basic` / `default` / `medium` | shared governed base through Eras 2–4 |
| `modern_v6t` | 5 | `e4_kers` | `v6t` / `basic` / `default` / `medium` | modern combustion resource; no electrical state claimed |
| `f1_2026` | 5 | `e5_solid_state` | `v6t_hybrid` / `active` / `f1_2026` / `medium` | historical V2 resource name; hybrid and active control remain absent |

Possible vehicle identifiers by enabled era are therefore:

| Era | Reachable governed identifiers |
| ---: | --- |
| 1 | `classic_v8_1960` |
| 2 | Era 1 plus `classic_v8_1970` |
| 3 | same as Era 2 |
| 4 | same as Era 2 |
| 5 | prior vehicles plus `modern_v6t` and `f1_2026` |

Eras 6 and 7 remain disabled. Their names or dormant upgrades do not authorize
an Active Aero, strategic-energy, or Pod/Drone vehicle resource.

## Executable contract

The exact fixture is duplicated at the two consumer boundaries:

- framework: `crates/pitgun-racing-contract/tests/fixtures/game_vehicle_unlock_contract_v1.json`;
- game: `src/engine/fixtures/game_vehicle_unlock_contract_v1.json`.

Both copies have byte digest
`sha256:4e26ede1e986fb302c6ba700136b9ece56c68d712b72aa7d962d317e807399e4`.
The framework test resolves every vehicle and component against Racing Catalog
1.3.0. The game test checks the public era limit, authored upgrade reachability,
selection order, and the same fixture digest.

At runtime, catalog hydration rejects a Simulation Pack missing any authored
unlock vehicle. The Racing Simulator independently rejects an unknown
`vehicle_id`. There is no fallback from an unresolved vehicle to an unrelated
generation.

## Evolution rule

A future Era 3 or Era 4 vehicle may replace this shared base only through:

1. a new immutable catalog release containing explicit engine, aero, chassis,
   and tire resources;
2. documented units, bounds, provenance, and model compatibility;
3. physical plausibility checks and deterministic native/WASM coverage;
4. a decision-surface campaign showing a useful, non-universally-dominant
   generation difference;
5. a coordinated update of both fixture copies and their digest.

Historical V1/V2 runs keep their original model and catalog identities.

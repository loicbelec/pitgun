# Racing Contract Inventory

This inventory freezes the ownership and compatibility baseline before Racing
schemas move from `pitgun-contract` to `pitgun-racing-contract` under issue
[#42](https://github.com/loicbelec/pitgun/issues/42).

## Explicit Racing Schemas

The current `pitgun-contract::racing` module owns the following domain types:

| Types | Current consumers | Decision |
|---|---|---|
| `RaceInput`, `CompetitorSpec`, `TuningSpec` | transitional Solver, `pitgun-policy`, game TypeScript mirror | Move to `pitgun-racing-contract`; document the optional game-only competitor fields |
| `RaceStint`, `CompetitorStintStrategy` | transitional Solver, game TypeScript mirror under the `CompetitorStint` name | Move to `pitgun-racing-contract` |
| `RaceOutput`, `StandingEntry`, `CompetitorStatus` | no external Rust consumer; the active Solver defines a different `RaceOutput`, while TypeScript uses the same name for that larger shape | Move as legacy Racing schemas, then reconcile or remove after the active Simulator contract is extracted |
| `VehicleClass`, `resolve_vehicle_class` | no consumer outside the defining crate tests | Move because they are Racing concepts, then reassess with the Solver extraction |
| `CircuitCatalogEntry`, `EngineCatalogEntry` | transitional Solver and WASM catalog API | Move to `pitgun-racing-contract` |
| `RunPackage` | no Rust consumer outside its defining crate; TypeScript uses the name with its larger Solver output shape | Move as a legacy Racing wire type, then reconcile or remove after consumer migration |

These types do not use generic contract primitives today. The migration must
still preserve an acyclic dependency direction if generic identities are added
later.

## Root-Level Legacy Contracts

Several older game contracts are defined directly in `pitgun-contract::lib` and
therefore appear generic even though their fields are game-specific.

| Types | Usage evidence | Decision |
|---|---|---|
| `SimulationContractV1`, `SignedSimulationContractV1` | active in `pitgun-authority` and its HTTP response | Move to `pitgun-racing-contract` |
| `ConfigRequestV1`, `TuningParam`, `CanonicalConfigV1`, `ContractLimitsV1`, `ConfigContractPayloadV1`, `ConfigContractV1` | no executable consumer remains; only defining-crate tests and a historical portal documentation entry remain, while the authority endpoint returns HTTP 410 | Remove during consumer cleanup rather than promote into the new crate |
| `MODEL_VERSION_V1`, `SCHEMA_VERSION_V1` | only used by the obsolete configuration-contract test | Remove with the obsolete configuration contracts |

Removal happens only after the compatibility PR has demonstrated that no live
workspace, service, game, or API consumer depends on these symbols.

## Repository Consumers

- `pitgun-solver` directly consumes Racing inputs, strategies, catalog entries,
  tuning, and vehicle classes.
- `pitgun-simulator` exposes the transitional Racing workload and output through
  its WASM facade.
- `pitgun-policy` directly validates Racing inputs, competitors, and tuning; its
  ownership moves separately under issue #43.
- `pitgun-authority` creates and signs `SimulationContractV1` responses.
- `pitgun-cli` consumes the Racing output indirectly through the Simulator.
- `pitgun-game` mirrors and extends several wire types in
  `src/engine/contract.ts`. In particular, its `RaceOutput` describes the active
  Solver output rather than the unused `pitgun-contract::RaceOutput`, and its
  competitor type accepts optional game-only fields. Reconciliation is tracked by
  [pitgun-game#36](https://github.com/loicbelec/pitgun-game/issues/36).
- `pitgun-api` has no direct consumer of these Rust or mirrored Racing schemas at
  the time of this inventory.

## Published Compatibility Fixture

`crates/pitgun-contract/tests/fixtures/racing_contract_v1.json` publishes
representative payloads and their canonical SHA-256 identities. It covers:

- nested Racing input, tuning, and stint strategy;
- standings with finished and DNF status variants;
- the legacy run package;
- vehicle and catalog payloads;
- the active signed simulation contract, including its exact signing bytes.

Both native and WASM tests consume this same fixture. The future
`pitgun-racing-contract` crate and the TypeScript mirror must reuse it rather
than create new expected values during migration.

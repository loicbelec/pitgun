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

## Reviewed Model V3 thermal candidate

The simulator also owns the candidate-only family resolver introduced by
`V3ThermalFamilyProfileCandidateV1`. It accepts only the exact reviewed
`pitgun.racing-v3-thermal-family-profile@1.0.0-rc.1` bytes, selects by
`vehicle_id` and fails closed for an unreviewed vehicle.

Native Rust uses `run_race_with_catalog_and_v3_thermal_family_profile` while
the browser-compatible boundary uses
`run_race_with_catalog_and_v3_thermal_family_profile_json`. Both attach the
same model, candidate, family and parameter-set identity to `RaceOutput`.

These APIs are an integration gate, not a published workload. The game and
hosted-verification services continue using their current catalog until a
later coordinated release authorizes the candidate model and profile.

## Component-composed Model V3 candidate

The next candidate boundary, Model `0.11.0`, versions thermal selection as
`V3PowerUnitThermalProfileCandidateV2`. It selects one reviewed thermal family
from the exact `installed_power_unit_id` of every competitor. Consequently,
two cars in one deterministic race may use different power units and thermal
parameters without changing their common vehicle-shell baseline.

Native Rust uses
`run_race_with_catalog_and_v3_power_unit_thermal_profile`; WASM and other JSON
adapters use the matching `_json` facade. `RaceOutput` records the exact
candidate, model, family, installed power-unit, parameter-set and reviewed
profile digest for every competitor. Unknown, missing, duplicate or mutated
bindings fail closed. Catalog `1.5.0` remains byte-for-byte vehicle-bound and
replayable. Catalog `1.6.0` provides the distinct non-production identity and a
strict component-capability resource. The same governed bytes let the
Simulator expose exact component lineage and let browser clients enable only
controls that the installed components really implement. Energy deployment
and recovery remain explicitly unavailable until their equations exist.

# Racing Model V3 thermal family profile candidate V1

Status: reviewed candidate. The Databricks gate passed and authorizes the
Rust/WASM integration step. This artifact is not a published Racing Catalog
and is not deployed to Authority, Verifier, game staging or production.

## Identity and evidence

| Field | Value |
|---|---|
| Candidate | `pitgun.racing-v3-thermal-family-profile@1.0.0-rc.1` |
| Candidate digest | `sha256:8aefd230da307e3439eef115fbfcd1117c8a8bbb1128c2c4b00138d6026f2f57` |
| Model | `pitgun.racing-v3-candidate@0.10.0` |
| Model digest | `sha256:cc1394a1ba52d83ddb9be6f6272729c29f87944969c41a21991d43997379e5cd` |
| Validation campaign | `racing-v3-thermal-refinement-validation-2026-v2` |
| Campaign digest | `sha256:5d336d09a1ed51086893285b7f2713730a6f9934872140831fe91fd9907ffc93` |
| Databricks validation | job `657259324536960`, run `176771852333040` |
| Read-only evidence review | job `242487850941668`, run `206668022732405` |
| Review digest | `sha256:b4747fe3193defec84889b90a755ad74417440d594433f2703c0b80a779aff42` |

The candidate is generated deterministically from the immutable campaign and
review artifacts. Its builder rejects incomplete family coverage, a modified
campaign payload or any verdict other than `PASS`.

## Reviewed profiles

| Family | Vehicle binding | Thermal decision |
|---|---|---|
| historical V8 | `classic_v8_1960`, `classic_v8_1970` | Retain the unchanged historical profile; cooling creates no artificial pace benefit. |
| modern V6T | `modern_v6t` | Adopt the reviewed profile with a `-3.0 °C` soft-limit offset. |
| F1 2026 | `f1_2026` | Retain the reviewed `adaptive-038` profile unchanged. |

Selection is deliberately keyed by `vehicle_id`, not by era alone. Both
`modern_v6t` and `f1_2026` are associated with era 5 but their reviewed thermal
responses differ. An unknown vehicle must therefore fail closed rather than
silently inherit a plausible-looking profile.

The simulator owns profile resolution. The solver receives exactly one
resolved `engine_thermal_resolution` for an execution and remains unaware of
catalog lookup or family-selection policy.

## Excluded result

The 130 kg fuel reservoir used by the corrected Databricks campaign is a
validation nuisance-control input. It prevented fuel depletion from masking
the thermal response over 52 laps; it is not a game calibration and is absent
from the candidate. Fuel mass, consumption and hybrid-energy behavior remain
owned by issue `#246`.

## Controlled release path

1. Integrate the candidate resolver in the Rust simulator and expose the same
   resolved profile to WASM (`#303`; implemented by the runtime integration
   candidate).
2. Prove native/WASM parity and fail-closed behavior for unknown vehicles
   (implemented by the portable runtime tests).
3. Publish a new versioned Racing Catalog candidate that references the
   integrated model and profile identities.
4. Promote the same catalog identity through Authority and Verifier.
5. Validate a complete weekend and verified leaderboard submission on game
   staging.
6. Promote to production only after the staging evidence is accepted.

No step is automatic. Each boundary must preserve the candidate, model and
catalog digests so that the resulting run can be reproduced and verified.

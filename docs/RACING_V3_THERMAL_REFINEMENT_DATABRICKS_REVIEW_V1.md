# Racing Model V3 thermal refinement Databricks review V1

Status: completed independent validation and human review. All three thermal
families pass the pre-registered gate. This document authorizes a versioned
profile candidate; it does not publish a catalog or game release.

## Evidence identity

The corrected `racing-v3-thermal-refinement-validation-2026-v2` campaign ran
from source revision `620b8b7` on Databricks job `657259324536960`, run
`176771852333040`. Its immutable manifest digest is
`sha256:5d336d09a1ed51086893285b7f2713730a6f9934872140831fe91fd9907ffc93`.

The review is pinned to these Delta snapshots:

| Table | Version |
|---|---:|
| `workspace.pitgun_calibration.campaigns` | 51 |
| `workspace.pitgun_calibration.experimental_runs` | 57 |
| `workspace.pitgun_calibration.experimental_metrics` | 56 |

The recorded human-review artifact has digest
`sha256:b4747fe3193defec84889b90a755ad74417440d594433f2703c0b80a779aff42`.

All 12 reserved Silverstone executions succeeded. They used seed `20260901`,
52 laps and the exact Rust Model V3 candidate `0.10.0`. No local parity error,
non-finite result or temperature above the 180 °C hard guard was observed.

## Reviewed response

| Family | Cooling | Time | Max temperature | Derated fraction |
|---|---:|---:|---:|---:|
| modern V6T | 0 | 6,288.832 s | 113.821 °C | 32.116% |
| modern V6T | 10 | 6,285.730 s | 98.682 °C | 0% |
| modern V6T | 20 | 6,288.910 s | 95.211 °C | 0% |
| F1 2026 | 0 | 5,774.153 s | 116.796 °C | 31.205% |
| F1 2026 | 10 | 5,765.968 s | 100.803 °C | 0% |
| F1 2026 | 20 | 5,768.936 s | 95.741 °C | 0% |

Ten cooling points are therefore the attainable interior optimum for both
authored thermal families. The modern V6T gains 3.102 seconds over zero
cooling and 3.180 seconds over maximum cooling. F1 2026 gains 8.185 seconds
over zero cooling and 2.968 seconds over maximum cooling.

The two unchanged historical V8 anchors remain below 93 °C, never derate and
receive no pace benefit from cooling. They do not acquire an artificial
thermal-management mechanic.

## Fuel correction and scope

The first immutable validation attempt used an inherited 80 kg screen input.
Both V6T families depleted it before the 52-lap workload, so that attempt is
retained as input-invalid evidence. V2 changed only the experimental reservoir
to 130 kg to stop fuel depletion from masking thermal behavior.

That value is not a game target, a catalog default or a real Formula 1
calibration. The production fuel and hybrid-energy contract remains owned by
issue `#246`.

## Decision

The family verdicts are `PASS`:

- historical V8 retains its unchanged profile;
- modern V6T advances with `soft_limit_offset_c = -3.0 °C`;
- F1 2026 retains `adaptive-038` unchanged.

The next step is to create a content-addressed, era-specific thermal profile
candidate. That candidate must then pass Rust/WASM parity and the normal
catalog, Authority, Verifier, game-staging and production release chain. No
promotion is automatic.

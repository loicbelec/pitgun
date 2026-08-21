# Racing Model V3 thermal Databricks review V1

Status: completed human review of immutable campaign evidence. This document
does not authorize a game or catalog release.

## Evidence identity

The `racing-v3-thermal-adequacy-2026-v1` campaign completed successfully on
Databricks run `655118884447136`, from source revision `90cbe75` and manifest
digest
`sha256:f6129f0799aaa849ae002534e5b148ae38be10f89f4edc283923b2da24fd54b2`.

The review is pinned to these Delta snapshots:

| Table | Version |
|---|---:|
| `workspace.pitgun_calibration.campaigns` | 47 |
| `workspace.pitgun_calibration.experimental_runs` | 55 |
| `workspace.pitgun_calibration.experimental_metrics` | 53 |

All 1,560 planned executions succeeded. The 720 local replay points reproduced
their Rust identities and seven retained metrics without one parity failure.
The campaign added 288 transition points and 552 Barcelona validations rather
than relabelling local selection evidence as validation.

## What the evidence says

| Family | Executions | Pathological | Verdict | Interpretation |
|---|---:|---:|---|---|
| historical V8 | 1,040 | 0 | `PASS` | thermal state remains safe; additional cooling costs pace and does not create a fictitious historical management game |
| modern V6T | 260 | 58 | `REFINE` | safe and engaged profiles exist, but no profile produces an interior cooling optimum on Monza, Singapore, and Barcelona together |
| F1 2026 | 260 | 62 | `REFINE` | `adaptive-038` produces the intended optimum on all three circuits, but reaches a 0.498225 derated fraction against the 0.50 pathology guard |

The pathological count is concentrated in zero-cooling runs. Maximum
temperatures remain below the 180 °C guard, while excessive derated duration is
the active failure mode. At ten cooling points the modern families generally
recover; at twenty points the additional drag normally costs pace. The equation
therefore expresses a real trade-off rather than making maximum cooling always
dominant.

The current one-node equation is not rejected. It can produce safe,
progressive, era-aware behavior, so the next action is scalar, family-specific
refinement. It is not yet a real Formula 1 calibration claim.

## Why no coefficient is promoted yet

`adaptive-038` is useful evidence, not a production profile. Its F1 2026 result
has only 0.001775 absolute margin below the declared derating guard. In the
modern V6T family it provides an interior optimum only on Monza; zero cooling
remains faster on Singapore and Barcelona. Selecting it now would overfit one
family and accept an insufficient safety margin.

The alternative `adaptive-044` increases the F1 2026 safety margin but misses
the Barcelona cooling optimum and provides no modern V6T optimum. A targeted
neighborhood between and around those two profiles is justified; another broad
random sweep is not.

## Gate to a game-usable profile

The next immutable experiment must:

1. authorize era-specific coefficients for historical V8, modern V6T, and F1
   2026 instead of forcing one compromise profile on all three;
2. pre-register a compact neighborhood around `adaptive-038` and
   `adaptive-044`, including an explicit safety margin stricter than the hard
   pathology boundary;
3. select locally on reviewed circuits, then use at least one new circuit or
   workload as untouched validation;
4. require a useful interior cooling optimum where thermal management is an
   authored capability, without making maximum cooling dominant;
5. produce a new versioned V3 profile only after a second human review.

After that gate passes, the profile can enter the normal release chain: Rust
and WASM parity, catalog candidate, Authority and Verifier compatibility, game
staging, then production. Databricks still proposes and records evidence; it
does not publish the game model.

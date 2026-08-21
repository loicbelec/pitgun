# Racing Model V3 thermal refinement — local selection V1

## Decision

The pre-registered local refinement returns **PASS** for the modern V6T
family. `soft-limit--3.0c` is the only candidate that creates an interior
cooling optimum on Monza, Singapore and Barcelona for both declared seeds.

This is a local selection decision, not a production calibration. F1 2026
retains `adaptive-038`; historical V8 parameters remain unchanged. The game,
catalog, Authority and Verifier are untouched until the independent
Databricks validation passes.

## What changed

Only `soft_limit_offset_c` changed, from `+0.9183673469 °C` in
`adaptive-038` to `−3.0 °C` for modern V6T. This moves the onset of power
derating earlier. Thermal capacity, heat generation, cooling capacity,
derating slope, minimum power and cooling drag remain byte-for-byte equal to
the reviewed anchor.

The experiment therefore answers a narrow question: can an earlier thermal
soft limit make a moderate cooling allocation useful without making maximum
cooling dominant? It does not claim that `−3.0 °C` is a measured real-world
F1 calibration.

## Evidence

- contract: `racing-v3-thermal-refinement-2026-v1`
- source issue: `#296`
- Rust model: `pitgun.racing-v3-candidate@0.10.0`
- executions: 126/126
- candidates: 7
- selection circuits: Monza, Singapore and Barcelona
- seeds: `42` and `99`
- workload: 18 laps
- cooling allocations: 0, 10 and 20 points
- result digest: `sha256:60315233d18f863e723f7a14c22cee11eb6c119cbe1d65455437096ab3afcb5a`

| Circuit | Seed | 0 → 10 cooling gain | 10 → 20 penalty | Derated fraction at 0 |
|---|---:|---:|---:|---:|
| Barcelona | 42 | 415 ms | 847 ms | 23.16% |
| Barcelona | 99 | 428 ms | 854 ms | 23.26% |
| Monza | 42 | 9,477 ms | 1,857 ms | 73.74% |
| Monza | 99 | 9,418 ms | 1,846 ms | 73.64% |
| Singapore | 42 | 1,638 ms | 617 ms | 40.24% |
| Singapore | 99 | 1,614 ms | 610 ms | 40.29% |

At ten cooling points, all six reviewed points recover to zero derated time and
remain below 99 °C. At twenty points, temperature falls further but elapsed
time increases because the extra cooling drag is no longer compensated by a
thermal benefit.

## Interpretation of zero cooling

The Monza result is deliberately severe: a player assigning no cooling spends
roughly three quarters of the run in some degree of power derating. This is not
classified as a numerical pathology because:

- every metric is finite;
- observed maximum temperature remains below 115 °C, far below the 180 °C hard
  guard;
- temperature and derating decrease monotonically when cooling is added;
- ten attainable cooling points restore non-derated operation;
- maximum cooling is not the fastest choice.

The severity is gameplay evidence that zero cooling is a poor setup. The
Databricks validation must still determine whether this behavior transfers to
an unseen circuit and full-race workload.

## Remaining gate

Silverstone (`gb-1948`), seed `20260901` and the full-race workload were not
executed locally. They remain reserved for independent Databricks validation.
That campaign must replay the selected modern V6T profile, retained F1 2026
profile and unchanged historical profiles through the packaged Rust probe and
return a family-level verdict.

Only a Databricks `PASS` may authorize a versioned era-specific candidate for
the Rust/WASM simulator and, later, staging. A failed transfer returns
`REFINE` or `STRUCTURAL_CHANGE_REQUIRED`; it must not be repaired by silently
changing the frozen local result.

# Native lap-boundary audit V1 — 22 November 2026

Source4364e19. **Exact-output cache qualified natively. End-state-held forecast
rejected as a general solution.** The remaining bounded/coupled look-ahead is
tracked in [#461](https://github.com/loicbelec/pitgun/issues/461).

## Computation removed, physics retained

The V2 global-minimum-grip braking forecast depends on current-lap fixed geometry,
mass, compound and pinned next driver controls. It was recalculated in every
tire/speed coupling pass although those inputs did not change. It is now cached
once per nonfinal lap. New lap/service/compound means a new cache. The energy-V1
endpoint-dependent forecast and the diagnostic held-state variant are recomputed.

There is no new approximation in this cache: physical inputs, equations, solver
roots/tolerances, native grid, observation rate and complete results stay exact.
No extra telemetry history is retained; the persistent cache value is one scalar.
Peak-memory reduction is not measured or claimed.

| Scenario | Old repeated forecasts | Cached forecasts | Cache hits |
| --- | ---: | ---: | ---: |
| Early V8,Monza5,cooling0 |31 |4 |27 |
| Modern,Barcelona20,medium |132 |19 |113 |
| Classic V8,Monza53,cooling2 |283 |52 |231 |

The last lap has no next-lap forecast. Counters describe attempted work, not
accepted physical events or future Engineer observations.

## Paired native cost

Seven independent serial processes per mode/scenario, order alternating. Other
native tests/campaigns had ended. The same binary and exact inputs/results were
checked for each run. The timer sums only `advance` calls: catalog/input resolution,
constructor validation, lap metrics, final hashing and JSON serialization are
outside it. Native setup in the first advance and diagnostic bookkeeping are
included. Other Mac activity is uncontrolled; no statistical throughput claim.

| Scenario | Recomputed median | Cached median | Less advance time |
| --- | ---: | ---: | ---: |
| Early V8,Monza5 |357.13ms |260.53ms |27.0% |
| Modern,Barcelona20 |1238.84ms |947.10ms |23.5% |
| Classic V8,Monza53 |3852.55ms |2930.31ms |23.9% |

This compares two equal-output work modes of the **same thermal/driver candidate**,
not old gameplay physics against the new physics. These are native Mac numbers,
not browser/WASM/VPS, total session latency, a10-car field or process RSS figures.
Raw7-sample timings and identities: [cost-v1.json](results/cost-v1.json),
[checksum](results/cost-v1.sha256).

## Why the pessimism cannot simply be removed

The frozen diagnostic changes only the forecast to hold predicted endpoint tire
state. That state is not a lower bound on grip over the following lap. The solver
retains its real braking/continuity checks, so this ablation can fail explicitly.

| Diagnostic case | Outcome with held endpoint |
| --- | --- |
| Early V8,Monza5,cooling0 | Rejected lap3,segment0: braking deficit429.55N |
| Early V8,Monza5,cooling2 | Rejected lap3,segment0: braking deficit425.34N |
| Classic V8,Monza53,cooling2 | Rejected lap9,segment0: braking deficit294.07N |

All three failures repeat exactly. Five other held-state scenarios complete,
including the cold/medium and soft→medium20-lap cases. Completion on those cases
does not make the assumption conservative or generally admissible.

The actual pessimism is visible in the classic V8 Monza5/cooling2 scenario:

| Mode | Driving speed at line,lap1 | Last300m brake work,lap1 | Total5laps |
| --- | ---: | ---: | ---: |
| Minimum grip |105.1km/h |1289.3kJ |615.944s |
| Held endpoint diagnostic |196.6km/h |430.8kJ |612.593s |

In the minimum-grip run the final-lap line speed is203.5km/h, when no next-lap
forecast is needed. Modern Silverstone similarly changes232.3→294.2km/h at lap1
under the ablation. These are one-car authored model cases, not measured racing
data. Legitimate circuit curvature/upcoming corners can require braking near
the line. The diagnostic's3.351s Monza difference is **not a qualified recoverable
performance gain**, because that assumption fails elsewhere.

## Qualification and reproducibility

- Eight scenarios ×3modes ×2passes =48 attempts; all cached/recomputed cases
  finish and match complete result digests and accepted lap metrics exactly.
- Whole diagnostic data, accepted prefixes, work counters and failures repeat
  exactly. Wall-clock cost is excluded from equality checks.
- Across all emitted laps, speed, engine temperature, tire surface/carcass and
  wear jumps against the previous committed post-service state are exactly zero.
  Rejected work is never published as a finished result or a corrected prefix.
- 56 prior driver inputs/outcomes, including5 frozen thermal baselines, remain
  exact. Three additional full-GP unchanged pre-service comparisons remain equal.
- 22 older historical/energy input/result/error outcomes remain exact, including
  the two expected Barcelona fuel failures.
- 171 native simulation attempts in this bank:48screen+56driver+3service+22history
  +42cost. This includes explicit failed attempts, not171successful finishes.
- 270 native tests pass: Authority30, Simulator135, Solver25+58, Verifier16+6.
  Strict scoped Clippy, default-member all-target check, formatting and diff pass.
  Tests include complete cached/recomputed/service equality, exact continuity
  and agreement of the two forecast semantics at manufactured constant grip.

Full [screen receipt](results/screen-v1.json) and [checksum](results/screen-v1.sha256)
bind source, binary, catalog, driver, physical and whole diagnostic input identity.
The explicitly hashed diagnostic wrapper is not an Authority/runtime admission.
Ordinary constructors select no diagnostic profile and emit no work counters.

### Workspace integration follow-up

CI exposed an older non-exhaustive event projection in the generic
`pitgun-solver` golden tests: it did not handle the previously added `Physical`
event variant. The initial local package-scoped270 tests and default-member
all-target check did not include that target. Test-only fix963f765 adds the
explicit projection; historical golden fixtures and simulation code are unchanged.

The expanded `cargo check --workspace --all-targets` passes, as does
`cargo test --workspace`:588 passed,0 failed,22 existing ignores (one live-Postgres
test and21 illustrative doctests). The14 generic Racing golden tests also pass
separately. The CI-scoped service/policy Clippy and formatting commands pass
locally. No new ignore or weakened assertion was added. Remote CI is rerun;
local success is not a claim that every remote job has completed.

These integration checks do not change or rerun the frozen performance evidence:
the only subsequent code edit is in the event projection test helper.

## Next physical gate

Keep the conservative cached trajectory as reference. Derive a meaningful bound
on reachable future tire states, or solve a bounded coupled horizon, from actual
surface/carcass/wear, mass and supported controls. Check terminal feasibility
**before** committing the current chunk, with recomputation confined to uncommitted
work. Qualify cold/hot/worn tires, service/controller boundaries, reserved real
career inputs, numerical error and cost. Never conceal an infeasible start by
trimming speed, enlarging tolerances or retrospectively changing accepted laps.

Actual real-career allocation/slider/tire screening and opponent fitting remain
after this gate. No balance/industrial precision, arbitrary process checkpoint,
new signed/native-WASM/export/Worker or current game/staging/prod activation is
claimed. Future fuel-mass gameplay remains recorded separately in the fuel lot.

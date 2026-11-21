# Native tire-window facts — 22 November 2026

Qualified source: `39d79426186885e96eb5417c014b449bb9b5675f`.
[Machine receipt](results/tire-v1.json), [digest](results/tire-v1.sha256),
[protocol/semantics](README.md#tire-window-increment--22-november-2026).

Eight exposed regression cases preserve the complete frozen physical result.
The five cases already in the derating bank also preserve their entire old
lap/pit/derating journal after removing only the newly selected tire facts.
Normal and pre-service execution, including serialization/restoration of both
detectors, pending queue and emitted journal, yield identical results and events.
The Solver remains in memory; this is not full process-checkpoint recovery.

| Case | Tire facts | Bands observed | Replacements |
| --- | ---: | --- | ---: |
| Early Monza cooling0 | 1 | Window | 0 |
| Early Monza cooling2 | 1 | Window | 0 |
| Modern Monza cooling0 | 1 | Window | 0 |
| Modern Monza cooling4 | 1 | Window | 0 |
| Classic Monza53 | 2 | Window | 1 |
| Barcelona medium20 | 1 | Window | 0 |
| Barcelona cold start35°C | 2 | Cold → window | 0 |
| Barcelona soft → medium, stop10 | 2 | Window | 1 |

There are169 records:133 completed laps,2 pits,23 derating transitions and11 tire
facts. In the cold-start case, window entry is first observed at140.354301s.
A replacement is a new set/stint even when its classification stays the same.
No hot-state crossing occurs in these eight actual cases; manufactured unit
trajectories cover both directions and inclusive window edges. The band is the
explicit diagnostic convention `temp_opt +/- sigma`, not experimentally identified
optimal grip, damage or retirement. No warning/decision/LLM budget is attached.

Each of two final passes checks22 historical/energy frozen outcomes and16 native
thermal executions (8cases×2modes):76 executions total in the final qualification.
Earlier uncommitted preview passes are superseded, not held-out validation.
253 native tests pass across Simulator, Solver, Authority and Verifier; scoped
strict Clippy, all-target Simulator compilation, formatting and diff checks pass.
The full suite was run on030a104; subsequent changes affect only the example's
display excerpt and README, with identical library sources. The final example
was rebuilt, compiled under strict Clippy and used for both recorded passes.

The new tests include invalid/overflow/stale input, exact boundaries, same-set
continuity, same/different-compound replacement and detector restore. An actual
dynamic service test changes the selected tire to hard at lap2, records the real
replacement and matches the physical result of the equivalent reference race.
The ten-car stream test selects both policies, retains old event content and
physical output, and checks IDs, release horizon and failed-journal behavior.

Two unranked visual excerpts retain at most600 native points per numeric channel
in the first600s, plus original event IDs/times. They are separate display fixtures,
not a new admitted exporter or a telemetry sampling-frequency change in physics.
The native medium/cold Barcelona case uses its circuit view; Monza engine events
use an explicitly flat presentation fallback. No graphical quantity generates
events. Performance of a future actual Worker/WASM is not measured here.

Default workloads, catalogs, equations, deployed WASM and admissions are unchanged.
Next: qualify the V2 export/runtime and full checkpoint, restrict facts at the
observation boundary, then admit the policies through Authority/Verifier and wire
actual game sessions. Deterministic relevance/persistence/grouping and supported
actions are required before these raw crossings trigger Engineer interventions.

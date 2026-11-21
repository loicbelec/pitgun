# First native physical-event increment — 21 November 2026

Source: `7ec598d8a8e29816033a47794f2cbf9227ea2292`. Receipt:
[native-v1.json](results/native-v1.json), with its [digest](results/native-v1.sha256).
Reproduction and semantic limitations: [README](README.md).

The detector adds derating entry/recovery facts without changing physics.
Twenty-two frozen historical/energy outcomes remain exact. The five thermal-V2
inputs below are reconstructed and matched by full canonical input digest; their
full physical result digests equal the previous frozen thermal screen.

| Frozen case | Lap events | Pit events | Derating starts | Recoveries |
| --- | ---: | ---: | ---: | ---: |
| Early budget 8, Monza, cooling 0, 5 laps | 5 | 0 | 4 | 3 |
| Early budget 8, Monza, cooling 2, 5 laps | 5 | 0 | 6 | 6 |
| Modern budget 20, Monza, cooling 0, 5 laps | 5 | 0 | 1 | 0 |
| Modern budget 20, Monza, cooling 4, 5 laps | 5 | 0 | 0 | 0 |
| Classic budget 15, Monza, cooling 0, 53 laps | 53 | 1 | 2 | 1 |

The journal contains 73 completed laps, one completed pit stop and 23 physical
transitions. An episode still active at the finish gets no fabricated recovery.
The six starts in the early cooling-2 case are raw crossings, not six alerts and
not evidence that it is thermally worse than cooling 0. Severity, persistence and
relevance are a separate future policy. The cooled modern case is a useful
negative control: a run need not produce any derating fact.

Normal and pre-service execution produce identical physical results **and** full
timed journals. The second mode serializes/restores detector state, pending queue
and emitted journal at the driving/service boundaries; its Solver stays in memory.
This is detector/journal continuity, not full process-checkpoint recovery. The
whole bank repeats exactly: 22 frozen outcomes plus ten thermal executions per
pass, two passes (64 native executions). These are exposed frozen regression
anchors, not new held-out physical validation or an AI-strategy benchmark.

247 tests pass across native Simulator, Solver, Authority and Verifier. The new
detector cases cover repeated/persistent episodes, terminal state semantics,
service recovery, restore, malformed input, overflow, forbidden future release
and retiming. A ten-car fixture checks the explicit mode, unchanged physical and
old-event content, occurrence IDs/release horizon, and sticky failure when an
advanced future cannot commit its event journal. Scoped strict Clippy, all-target
Simulator compilation, formatting and diff checks also pass.

Default constructors/streams, physical equations and catalogs are unchanged.
Neither current WASM/browser workloads nor Authority admissions enable this mode.
The native opt-in mode selects existing player-aligned release. Hidden rival
physical facts still require a capability-limited view before any provider/UI
delivery. This increment makes no claim about graphical export, full saved-run
recovery, remote attestation, runtime performance or deployment readiness.

Next: native tire-window facts; frozen relevance/visibility policies; then the
new runtime/export/checkpoint/Worker and Authority/Verifier admission, followed
by Pit Wall presentation. Automatic fuel/DNF, hybridization, incidents, VSC and
weather remain separately gated by their actual models.

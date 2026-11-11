# Interactive simulation control — architectural direction

11 November 2026 · local delivery 2 · framework #415 / game #316.

Pitgun's objective is a deterministic simulation that an external controller can
influence during execution. Racing is the first domain. Controllers may be humans,
LLMs, other ML models, deterministic policies or optimizers. A second-domain proof
will test which mechanisms are reusable before freezing a public contract. Energy
is a candidate; the owner may provide a more developed example. The current work
does not prescribe a network model or require a battery demonstration first.

## Ownership

| Layer | Owns |
| --- | --- |
| Shared execution/lifecycle | Run/runtime/input identity, logical ordering, observation/action binding, pending/resolved state, durable journal and replay |
| Domain | Physical state, event meanings, units, valid actions, legal application frontiers, validation and effects |
| Host/controller adapter | Scheduling controller calls, provider timeout, user interaction, budgets and persistence before advancing affected work |
| Presentation | Cursor, playback speed, compact messages, history and evidence navigation |

Common code must not assume laps, pits, fixed-duration steps, one actor or
independent competitors. Racing's new service primitive stays in its own Solver;
no speculative cross-domain trait or public universal wire schema is added now.

## Three distinct coordinates

- **Observation:** only facts available through an explicit simulation horizon.
  Event occurrence time differs from the compute frontier that releases a batch.
- **Deadline:** a logical window closing before affected work is committed.
  Provider wall-clock timeouts are host outcomes, recorded as explicit fallbacks;
  they are not physical simulation time.
- **Effect:** a domain-validated future frontier, recorded in the application receipt.
  A stale response cannot rewrite history or silently move to another opportunity.

Playback must be able to consume the already solved unaffected prefix while the
controller is pending. The engine must stop before any affected future work. If
no valid response is available at the logical deadline, the host records the
explicit keep-plan fallback before allowing that work. Runtime, initial inputs,
events and accepted action sequence determine replay; replay never recalls a
historical controller. Source labels/hashes alone are not remote attestation.

## Racing first slice

Keep the starting plan and allow keep-plan or pit at the earliest eligible
service boundary, with an authorized tire. Exact per-car events provide evidence;
Racing rules select strategically relevant events and group them into episodes.
Most events produce no message. One compact bubble contains a short reason,
existing small gauges, deadline and two actions; detailed evidence stays in Debrief.
Temperature/wear alert thresholds remain deferred pending their model calibration.

The event filter can also evaluate periodically; event-driven scheduling is not
mandatory for every domain or controller. Version filtering thresholds, dedup,
cooldown and reasons alongside the run. The selected policy must not inspect
future rival plans, hidden trajectories or a final classification.

## Current increment versus remaining work

Implemented locally: optional versioned native event timing, horizon-safe event
release, physical chronology in the Pit Wall and native before-service stepping.
The native primitive emits driving once, waits for resolution, commits service,
then continues without recalculating that driving prefix. Existing default
workloads and published parity bytes remain unchanged.

Still required: domain orchestration and versioned experimental transport for the
new primitive; opening/closing decision windows during playback; durable pending/
proposal/receipt/deadline recovery; same observations for manual/reference/Gemini;
precise versioned relevance policy; full-GP/failure/lifecycle acceptance; initial
Authority capabilities and terminal ordered action-journal Verifier evidence.

The existing v1 development Pit Wall still pauses at configured completed-lap
windows. Native before-service stepping is not yet exported through its handles
or activated in that UI. Do not silently reinterpret old v1 journals.

## Acceptance for the next slice

1. One known rival event opens one bounded window with a stable reason/evidence ID.
2. Cars continue along solved unaffected paths while a response is pending.
3. A valid early response selects the earliest eligible service; an expired window
   records keep-plan. A late/conflicting response is rejected without retargeting.
4. The exact observation, deadline resolution and effect receipt are durable before
   affected continuation; reload/replay do not ask historical choices again.
5. Continuous/interrupted runs reproduce canonical results and event order. Full-GP
   and memory measurements are scoped separately from short deterministic fixtures.
6. Existing v1 saved binaries, governed default streams and normal careers retain
   their contracts; new local mode and compatibility are explicit.

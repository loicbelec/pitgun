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
release, physical chronology and before-service stepping through the existing
experimental handles. Explicit `pitgun.racing-service-window/v1` configuration
freezes the observation after lap k, emits unaffected driving k+1 once, waits for
resolution, commits service, then continues without recalculating that prefix.
Opening, deadline and service frontier are distinct; proposals record logical
decision time. Expiry records Keep plan; late responses cannot retarget service.
Existing default workloads and published parity bytes remain unchanged.

The separate game `/dev/pitwall-control-live` bank keeps cars moving during a
manual window and durably records observations, proposals and receipts before
affected continuation. Manual, expiry and pending-reload Chrome journeys pass.
Reload reconstructs the same logical deadline but replays the pending presentation
from its opening, rather than preserving an exact animation frame or wall-clock
budget. The old `/dev/pitwall-control` bank still pauses at completed-lap windows;
its saved binaries/journals retain their original semantics and bridge.

A separate `/dev/pitwall-control-events` bank now opts into a versioned Racing
rival-pit policy. Released nearby service completions are grouped at committed
player-lap checkpoints, with own-plan eligibility, deduplication, cooldown and a
window budget. The frozen trigger carries exact evidence IDs and policy identity;
it is not inferred by React. Existing scheduled five-lap banks remain regression
fixtures. The event fixture is seven laps, without provider integration. Exact future service clocks are host scheduling metadata, separate
from past physical features: an explicit provider feature projection is required
before sending observations to a model. Host-declared logical time is not remote
attestation. Full-GP/failure/lifecycle acceptance, relevance-threshold calibration,
controller adapters, initial Authority capabilities and terminal ordered
action-journal Verifier evidence remain required before career/ranked activation.
See the [game connection evidence](https://github.com/loicbelec/pitgun-game/blob/feat/controlled-racing-worker/docs/qa/LIVE_SERVICE_WINDOWS_2026-11-11.md).

## Acceptance for the next slice

1. One known rival event opens one bounded window with a stable reason/evidence ID
   (implemented in the separate local event-policy bank; thresholds remain experimental).
2. Cars continue along solved unaffected paths while a response is pending.
3. A valid early response selects the earliest eligible service; an expired window
   records keep-plan. A late/conflicting response is rejected without retargeting.
4. The exact observation, deadline resolution and effect receipt are durable before
   affected continuation; reload/replay do not ask historical choices again.
5. Continuous/interrupted runs reproduce canonical results and event order. Full-GP
   and memory measurements are scoped separately from short deterministic fixtures.
6. Existing v1 saved binaries, governed default streams and normal careers retain
   their contracts; new local mode and compatibility are explicit.

## Full-distance local qualification — 12 November 2026

The separate game `/dev/pitwall-control-gp` bank exercises Monza 53 laps with ten
fixed independent cars, varied pit plans and enough pace difference for lapped
relative progress. A native probe accepts the same versioned game fixture path;
Keep/Timeout preserve fixed-plan output, the first pit request matches an equivalent
predeclared plan, and applied-action replay reproduces each native result.

Direct WASM and durable Worker paths qualify the same GP, including late refusals,
pending document reload, receipt-write failure, cancellation and deterministic
fuel-exhaustion failure. Actual desktop/mobile Pit Wall journeys cover two live
windows, pending/completed reload and expiration. Source facts and paths remain
bounded by their released player horizon. The existing published/runtime artifacts
are unchanged; this adds qualification inputs and adapters, not physics changes.

These are local control/recovery gates, not competitive threshold calibration,
provider attestation or hosted career/ranked admission. Extreme admitted setup
inputs can exhaust fixed fuel before the finish: this remains an explicit failed
session, linked to #414, not a published result. Memory remains high even without
the UI and is tracked in #430 / game #315. Exact provider feature/clock projection,
full rival-lap Compare, initial Authority capabilities and terminal ordered-journal
Verifier evidence remain separate gates. Gemini can be connected experimentally
after its observation/budget contract is specified, before ranked activation.

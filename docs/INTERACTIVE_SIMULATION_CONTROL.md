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

## Physical-event direction — 21 November 2026

Owner requirement: derating belongs in the event channel used by Pit Wall and
controllers, alongside future incidents caused by aggressive driving. This section
specifies the proposed extension; it does **not** add event variants, thresholds,
an incident model, a new wire/runtime identity or UI behavior.

**Implementation update, same day:** the first native opt-in increment now adds
derating entry/recovery facts in `physical_events.rs` and the existing session
journal. The design discussion below records the starting point. See
[the native event qualification](../experiments/racing_physical_events/README.md)
for exact semantics, repeat procedure and limitations. Physics/default stream
outputs remain unchanged; no current browser workload selects this mode.
The new payload is versioned, but this is not a new admitted wire/runtime identity.
Existing decoders require an explicit updated contract before consuming it.

### Reuse and ownership

`RacingSessionEventV1` currently carries `LapCompleted`, `PitStopCompleted` and
`DriverInstructionApplied`. Optional `RacingEventTimingV1` already separates
simulation occurrence from release at an observed compute frontier. The shared
contract also has generic frame events. Reuse these transport/journal primitives
with a versioned domain payload and an explicit new profile; do not create two
independent authoritative event logs or silently change old saved streams.

The Solver owns physical state and validated transitions. Racing-owned event
detectors consume the **accepted native trajectory/state**, before graphical
resampling, and keep their detector state between chunks. Numerical coupling/root
iterations are provisional work: they must never produce committed physical
events. Racing Simulator owns consequences involving the race as a whole, such
as neutralization, and legal command-effect frontiers. Generic framework code
owns identity, ordering, release, persistence and replay without understanding
engines, tires or laps. Presentation owns icons, messages, playback and navigation.

### Proposed event families

| Family | Useful facts/transitions | Prerequisite/status |
| --- | --- | --- |
| Engine | Thermal derating begins/ends; a significant worsening of an existing episode | Native V2 has temperature/derating samples, not these discrete records yet. Distinguish available power from delivered power. |
| Tires | Surface exits/returns to its thermal window, wear crosses a declared milestone, modeled grip loss becomes significant | Surface/carcass/wear exist in V2; warning thresholds require calibration/versioning.100% wear is not automatically a puncture or retirement. |
| Fuel | Reserve threshold reached, actual fuel exhausted; separately labelled range-risk estimate | Current exhaustion is a solver error. Domain DNF semantics and automatic sizing remain #445. An autonomy forecast is an estimate, not an observed physical fact. |
| Energy | Charge/reserve threshold crossed, recovery/deployment limit reached, electrical thermal protection | Only after battery/regen/limits exist (#246). |
| Race/strategy | Existing completed laps/pits/instructions; future observed position changes or race-control transitions | Derived by Racing from released committed facts, not the React cursor or hidden future plans. Do not invent pit-entry precision from an abstract pit penalty. |
| Incidents | Modeled driving error, spin, collision/damage or retirement, when supported | Aggressiveness must affect an explicit incident model first. No incident inferred or invented by an LLM from a slow lap. |
| Conditions | A scenario/weather state changes, track conditions change, VSC begins/ends | Only after the corresponding scenario and physical/race consequences are implemented. |

Exact names and payload schemas remain to review. A first bounded implementation
should cover thermal derating episode entry/recovery, then tire-window transitions;
fuel events follow sizing/exhaustion semantics. Existing lap/pit/instruction events
remain the reference. Incidents, neutralization and weather are separate modules,
not fabricated notifications added to this thermal slice.

### Facts, relevance and presentation

Continuous quantities remain numeric channels. Discrete records capture meaningful
transitions; event definitions must state whether they denote an exact physical
mode change or a sustained diagnostic condition. Use declared, versioned thresholds,
hysteresis and persistence for diagnostic episodes, without changing the physical
derating law. Do not issue an event for every changing temperature sample.

A separate deterministic relevance policy selects/groups released events into
episodes, with deduplication, cooldown and a bounded decision/message budget. Most
events need no bubble or LLM call. An alert may explain a condition without opening
a decision window if no valid action exists. Current thermal V2 does not admit
dynamic driver-control schedules; do not offer an actionable cooling/pace command
just because an engine event was detected.

Internal audit facts and controller-visible facts are different views of the same
journal. A rival's actual engine temperature, fuel or future strategy must not be
given to a player/LLM unless its observation capability explicitly permits it.
An inferred rival performance issue belongs to the analysis/estimate layer, not
an authoritative claim about a hidden physical cause. Freeze both visibility and
relevance policies with the run, and bind selected episodes to source event IDs.

### Determinism, causality and audit

Each new record needs a stable run-scoped occurrence ID/order, kind/schema,
entity, simulation occurrence coordinate/time, release frontier and typed numeric
payload with units. The run manifest binds model/input/policy identities. Declare
native detection-time precision; ordering must be stable even when rounded times
coincide. Repeated starts/stops of the same kind within one lap must have distinct
IDs. Wall-clock time and rendering frequency must not define physical events.

Given model, initial/scenario inputs, seed and accepted actions, events must replay
in the same order. Seeded incidents remain deterministic: pin the risk law and
random-stream identity/ordering, and ensure chunking, UI speed and controller
latency cannot alter their draws. Replay uses recorded accepted actions rather
than calling the original LLM again. This is reproducibility, not remote attestation.

An incident that triggers VSC produces a race-state consequence. Every affected
car must be advanced under that state before affected future work is committed;
painting a VSC banner over an already solved race is insufficient. Current
independent-car/prefetch assumptions must be qualified for coupled race events.
The controller observes only released history and may act at a declared future
legal boundary, never rewrite a computed prefix.

Acceptance: continuous/chunked/reloaded execution yields the same events; observation
export rates leave event identity/order unchanged; provisional passes emit no
duplicates; threshold episodes do not chatter; future/private facts stay outside
unauthorized views; recording a fact is distinct from opening a window; full
event/decision/effect evidence is replay-verifiable. The first observation-only
event slice must preserve physical results when no action is taken. Old streams
retain their schemas, outputs and saved runtime identities.

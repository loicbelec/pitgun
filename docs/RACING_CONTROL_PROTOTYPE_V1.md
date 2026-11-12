# Racing control — local prototype, delivery 2

7 November 2026. The owner authorizes starting delivery 2 locally. Delivery 1's
Chrome/Safari interruption checks pass, but its performance, memory, mobile and
Engineer release gates remain open. Starting this independent framework work is
not approval to merge, activate or deploy either delivery.

Tracking: [framework #415](https://github.com/loicbelec/pitgun/issues/415),
[draft PR #429](https://github.com/loicbelec/pitgun/pull/429).
The approved [three-delivery plan](https://github.com/loicbelec/pitgun-game/blob/feat/signed-buffered-recovery/docs/design/PROGRESSIVE_SIMULATION_DELIVERY_PLAN.md)
remains authoritative. This document describes the **first native increment**,
not completion of #415 or a stable public Engineer Contract.

## Implemented boundary

`pitgun_racing_simulator::control_prototype::LocalControlledRacingSession` wraps
the existing stateful V3 fuel-contract session. It pins the initial request,
catalogue release, model identity, initial driver timeline, execution identity,
declared runtime digest, decision laps and tyre allowlist in a canonical run
digest. Inputs and these capabilities cannot change after start.

At a configured one-based completed-lap boundary, `advance()` returns a frozen
player observation instead of computing another lap. Repeated calls return the
same observation, without advancing any Solver or duplicating the observation
journal entry. A valid proposal closes exactly that boundary. A rejected proposal
leaves it open; the host can submit a correction or an explicitly identified
`keep_plan` fallback. The module has no timeout or inference of its own.

| Action | Racing semantics |
| --- | --- |
| `keep_plan` | Preserve the existing remaining strategy. |
| `pit_next_lap { tire_id }` | After `k` completed laps, service at the **end of lap `k+1`**. Complete that lap with its existing tyre. Add the existing catalogue pit loss, reset wear/tyre temperature and launch/gear continuity through the existing physical service transition. Use the selected tyre on laps `k+2` through finish. Replace all still-future stops; retain completed service. |

Decision laps must be strictly increasing, from 1 to `total_laps - 1`. A pit is
unavailable after `total_laps - 1` laps because service at the finish would leave
no next stint. `keep_plan` remains legal there. There is no action accepting an
arbitrary lap number, setup, aggression or physical coefficients.

The Solver adds only a pending-pit slot consumed after its previous yield and
before work on the next lap. There is no restart, recomputation or modification
of the completed physical prefix. Racing owns tyre resolution, capability checks
and matching updates to stint metadata and final service-time accounting.
Existing batch, static timelines, model/catalogue releases and authorised dynamic
entry points do not call this experimental path.

## Observation, proposals and journal

The observation contains run/domain/version, sequential decision identity,
completed/total laps, player identity, last/cumulative time in milliseconds,
remaining fuel in kilograms, wear fraction, tyre/engine temperatures in Celsius,
the next-lap tyre, remaining planned stops and delegated capabilities. These are
the player's resolved state and own plan, not simulated rival data.

Although the underlying field advances by lap count, rivals have different
physical clocks. No rival telemetry, ranking, reconstructed trajectory, future
result or RNG state is disclosed by this prototype's observation. A later rival
comparison needs a separately specified common physical horizon.

Proposals bind the exact run, decision number, completed-lap boundary and canonical
observation digest. Typed actions reject unknown fields/kinds; unsupported versions
fail deserialization. A key of 1–128 bytes makes an accepted request idempotent:
identical retry returns the original receipt, including after advancement; altered
content under that key conflicts. A fresh key cannot reopen a closed boundary.
Unknown/undelegated tyres, stale bindings and a pit at the finish are rejected.

The journal separates observations, proposals, rejections, applied decisions and
duplicate acknowledgements. Source is explicit: reference policy, manual input,
or a host-selected fallback with a typed reason. The prototype fallback only keeps
the plan; it does not pretend to be a controller decision. Source labels are not
remote provenance attestations.

## Replay and limits

`replay()` reconstructs the initial session and consumes **applied decisions**,
regenerating each observation and validating its binding and application receipt.
It does not call a controller. Missing, extra, reordered, differently bound or
tampered receipts fail. Repeated waiting/pull cadence does not affect stream bytes
or results. Wall-clock waiting has no simulated duration.

The runtime hash is supplied by the local runner, not verified by the library or
attested remotely. The probe hashes its actual executable and replays in that same
process. A future archive runner must retain/check the actual artifact; copying
a digest label to another binary does not pin that binary. Exactness is currently
demonstrated **within this native runtime**, not across arbitrary native/WASM
floating-point implementations.

The journal is currently in memory. The caller must persist inputs, exact runtime
and catalogue artifacts, observations, proposals, selections and applied records.
The probe report retains input/configuration, catalogue identity, journal, result
summaries and full-result hashes; it is not a complete durable benchmark archive.
There is no durable mid-decision recovery, public endpoint, quota enforcement,
Gemini call, hosted control authority or leaderboard submission in this increment.
The host will need explicit proposal/resource limits before exposing transport.

The generic contract crate is unchanged. These experimental Racing types remain
domain-owned until [battery #416](https://github.com/loicbelec/pitgun/issues/416)
proves which lifecycle/envelope elements are actually shared. No energy-network
or RTE operational model is implied.

## Reproduce the native proof

From this framework checkout:

```sh
cargo test --offline -p pitgun-racing-solver -p pitgun-racing-simulator -p pitgun-racing-policy
cargo run --offline -p pitgun-racing-simulator --example racing_control_probe > /tmp/pitgun-control-report.json
cargo check --offline -p pitgun-racing-simulator --target wasm32-unknown-unknown --features wasm
```

The five-lap Monza fixture is a **protocol fixture**, not a Grand Prix benchmark.
It compares its original soft/medium plan (service after lap 3) with a reference
policy requesting hard tyres after lap 2, then replays the latter. The policy is
deliberately simple and makes no optimality or model-quality claim.

The [observed native report](qa/racing-control-native-2026-11-07.json) records:

| Check | Observed result |
| --- | --- |
| Baseline service / total | Lap 3 / 502,538 ms |
| Controlled service / total | Lap 2 / 502,518 ms |
| Baseline canonical result digest | `sha256:40cb3934966e237555f9711c7b1c4c89e98877150df45a6644387ce7cfd99386` |
| Controlled and replay digest | `sha256:cf044eafcf41fec2eba2208c81546e427e369873b788e7c1e6173ef42fc2490b` |
| Catalogue | Racing 1.9.0, manifest `sha256:30621c0a2c6ff232eacecfb92737372e0f8766e121b45c8b2fb48cc762ca49ac` |

Eleven new native tests cover freeze/cadence, baseline parity, real service/reset,
equality with an equivalent predeclared plan, a second pit preserving past service,
idempotency/conflicts, rejected bindings/actions, explicit fallback, finish limits,
strict action parsing, altered replay inputs/records and ten-car isolation. The
ten-car case changes the player's strategy while preserving every rival's physical
times, rather than asserting that rank/gaps must stay unchanged.

Observed validation on 7 November: **124 tests pass** across Racing policy,
Simulator and Solver (including 11 new control tests); WASM compilation with the
`wasm` feature passes. Strict Clippy passes for the Simulator library, its Solver
dependency and the new probe. The broader `--all-targets -- -D warnings` command
stops on four existing `field_reassign_with_default` diagnostics in
`crates/pitgun-racing-solver/tests/physical_kernel.rs` (lines 985, 992, 999, 1016).
Those unchanged test fixtures are not silently rewritten or reported as a green
strict all-target check. No browser or WASM execution conformance is claimed here.

## Remaining delivery 2 increments

1. **Native boundary:** this prototype and executable deterministic reference;
   local review in progress. Not a new governed model release.
2. **WASM/host transport:** [game #316](https://github.com/loicbelec/pitgun-game/issues/316),
   explicit experimental JSON/handle boundary and Worker
   state machine, conformance vectors, bounded messages, acknowledgement separate
   from permission, buffer stopping before open decisions, durable journal and
   reconstruction to the next unresolved window without historical controller calls.
   Compilation for WASM alone is not that acceptance.
3. **Governed execution:** version initial delegation/permissions and terminal
   applied-journal evidence, Authority and Verifier reconstruction, integration and
   negative tests. Existing signed driver-mode permissions must never silently
   authorise pit/tyre changes. Hosted/ranked acceptance remains separate from local.

Only after these steps should delivery 3 connect Gemini to actions and contextual
messages. The shared domain-neutral contract is frozen later, after the battery
proof. Delivery 1 remains independently gated; production and branch cleanup are
outside this local increment.

## 9 November — experimental JSON/handle transport

`controlled_stream.rs` exposes a separate local-only start/pull/propose/complete/
release facade with `pitgun.racing-control-*/v1` response envelopes. Start takes
opaque request, exact catalog bundle, initial timeline and control configuration
JSON. Pull returns either a canonical stream batch or a frozen player-only
observation **with its Rust-generated digest**. Clients copy that digest rather
than rehashing floating-point observations in JavaScript. Propose returns the
applied receipt or an error, leaving a refused window open. Complete consumes the
terminal handle and returns RaceOutput, the journal and the canonical result
digest generated in this runtime. Release discards state without results/evidence.

Resource limits are experimental host guards, not published physics changes:
4 simultaneous handles per thread/Worker instance; 128 laps; 16 competitors;
64 decision windows; 16 delegated tires; explicit 1–20 Hz; request256KiB,
catalog16MiB, timeline64KiB, configuration16KiB and proposal4KiB.
A session accepts at most256 submission attempts (including malformed JSON),
which bounds its proposal/rejection/duplicate journal. Waiting pulls add no entries.
Reaching the budget requires cancellation/reconstruction, not an implicit fallback.
Full output telemetry still scales with session length; this is not the deferred
memory-retention fix.

Three additional native facade tests cover frozen repeated pulls, wrong bindings,
identical retry/conflict, terminal consumption, size/handle/submission limits and
incomplete cancellation. The browser transport is tracked in game#316 and is not
authorised or ranked by these exports. The original signed entry points and generic
contract crate are unchanged. `racing_control_probe --browser-fixture` additionally
emits opaque browser inputs and the same immutable catalog bundle for executable
cross-runtime protocol checks. Its native runtime identity must be replaced with
the actual browser artifact digest when that browser session is started; native
and WASM are different runtimes, never mutually attested by that fixture.


### Executed browser proof

Game#316 now carries these handles through a separate bounded module Worker.
The [9 November report](https://github.com/loicbelec/pitgun-game/blob/feat/controlled-racing-worker/docs/qa/RACING_CONTROL_WORKER_2026-11-09.md)
records seven rejected-command vectors, one unconsumed batch, physical service
at lap2 instead of lap3, and exact full-result replay in a new Worker using the
same actual WASM artifact and recorded applied proposals. Native and WASM identities
are distinct; their integer lap/total time deltas are zero in this fixture, with
a declared1ms tolerance, while full telemetry hashes differ across runtimes.
127 native Racing tests pass, including the three facade tests. Strict library/
probe Clippy and WASM release compilation pass; the earlier all-target fixture
warning limitation remains. Game651 tests, default build and Chrome proof pass.
These are local five-lap protocol checks, not a Grand Prix or hosted acceptance.

The host waits for observation, proposal, rejection and receipt checkpoint hooks.
The fixture stores them in memory; durable persistence and reload reconstruction
remain next. No historical controller is called during the fresh-Worker replay.
No Gemini, control authority, public protocol freeze, ranked result or deployment
is enabled. The normal governed/batch APIs and generic contract crate stay unchanged.

### Common physical-time presentation (11 November)

The local controlled session now opts into a player-horizon presentation path.
The governed/default stream remains on its original completed-lap scheduler and
published parity vectors. No generic contract, physical coefficients, exports or
control configuration fields change.

At each emitted player lap, rivals with fixed independent inputs are advanced
only until their solved path brackets that player's cumulative time, or their
own finish. The player is never prefetched: a pending observation still freezes
all affected player physics. Rival continuity records are queued for the usual
lap-by-lap collection; their heavy solution arrays are discarded after copying
the time/distance path. Unemitted path tails retain the exact boundary bracket.
The terminal canonical result is still collected from the same Solver sessions.
This assumes the current independent competitors: it must be revisited before
adding rival reactions, coupled traffic or shared mutable race conditions.

Grid trajectories contain only prefixes through the player horizon. Boundary
interpolation is presentation data derived from native solved brackets, not new
physics. Running rows end at the common time; a finished rival retains its actual
finish time and distance. Service time stays stationary. A future segment used
to bracket the cursor is never delivered past that cursor. Rows may therefore
represent different laps; their live positions sort unwrapped solved distances,
while terminal standings still belong to RaceOutput. Existing lap events retain
their completed-lap delivery semantics; they are not new real-time rival alerts.

Focused tests cover faster/slower/finished rivals, stationary service, contiguous
prefixes, no leaked future points, compact queued continuity, no player look-ahead,
and byte-identical native results versus the unchanged batch path. Exact runtime
hashes still distinguish old and new implementations. Hosts must resume an old
journal with its saved binary, not substitute this implementation by version label.
Full-GP/lifecycle qualification and Authority/Verifier delegation remain open.

### Physical events and native before-service primitive (11 November)

See [interactive simulation ownership and lifecycle](INTERACTIVE_SIMULATION_CONTROL.md).
Racing semantics remain domain-owned; a second-domain example is to be selected
with the owner before public contract freeze. Neither host wall time nor replay
speed becomes a physics input.

New local aligned streams add optional `timing` metadata to existing typed event
payloads: schema `pitgun.racing-event-timing/v1`, run-scoped stable event digest,
`occurred_at_us` on the actor's physical clock, and `released_at_us` on the player
compute frontier. Times round to nearest microsecond. Default/governed streams
omit this field, retaining their published byte vectors. Logical stream ticks
still order batches; they are not elapsed race time.

The engine captures events when a solved lap is produced, including prefetched
rivals. It queues future facts until occurrence is within the player horizon and
releases them in stable physical-time/phase/digest order. An instruction applies at lap
start; a completed lap crosses before optional service; a completed pit follows
service time. Slow-rival events later than the player's finish remain unreleased
in this player-horizon progress stream; terminal standings are still separate.
No accident/VSC/weather event is invented by this increment.

`ResolvedSimulationSessionV3::new_with_service_boundaries` enables a separate
native-only experimental path:

- `advance_service()` first returns `BeforeService` with the solved driving path,
  fuel/wear/thermal state and cumulative time before any current-lap pit service.
- Further pulls fail without polling while that boundary is unresolved.
- `resolve_service_boundary(None)` keeps the plan; an authorized tire selected by
  the caller replaces remaining stops with a stop at that boundary. The last-lap
  restriction, invalid tire and closed-boundary checks precede mutation.
- The following `Lap` step commits service/state with an empty solution: the path
  was emitted once. Subsequent driving resumes from that state. Default `advance`
  and service stepping reject mixed API use rather than silently bypassing a gate.

Two tests preserve exact canonical native output for keep-plan, and match an
equivalent fixed pit plan after a real service change. They check unchanged driving
prefix, physical service time, reset state, replaced remaining stop, unresolved
pulls, invalid/closed/final commands and mixed API rejection. A separate event test
covers faster/slower clocks, prefetched instruction events, withheld future facts,
unique ordering and distinct crossing/service times. All132 tests across Racing
policy/Simulator/Solver pass, plus strict library Clippy and formatting.

This primitive is not yet used by `LocalControlledRacingSession` or exported by its
JSON handles. The v1 Pit Wall still pauses at completed-lap decisions. Domain
orchestration, pending-window/timeout journal semantics and transport/recovery
must be adapted explicitly before continuous-play decisions can be claimed.

### Opt-in service-window handle extension

The same experimental JSON handles now accept an explicit optional configuration:
`service_windows: { schema_version: "pitgun.racing-service-window/v1",
closing_margin_us: 5000000 }`. Omission preserves the original completed-lap
protocol and serialized observation/proposal bytes. No new ABI export or generic
Racing-independent contract is introduced.

For a configured observation after completed lap k, Racing freezes the committed
state at k, then emits the independently solved driving prefix for k+1 before
requesting a response. `control_prefix: true` marks that single pending-window
batch so host presentation backpressure cannot prevent delivering the window.
It grants no permission to advance affected physics.

The immutable observation's optional `window` binds its version, exact opening,
logical deadline, pre-service physical frontier and target service lap. The next
lap's already solved state/telemetry is not copied into the observation. Optional
`decision_at_us` on the proposal is required in this mode: a selection must fall
within [opening, deadline); a timeout fallback must keep the plan and carry the
exact deadline. Source/run/observation/idempotency checks remain. This is a local
host-declared logical decision time, not remote clock attestation or controller
provider-budget enforcement. Authority/Verifier activation remains separate.

The receipt still binds the proposal and effective pit lap. It is persisted before
the service commit is pulled. Non-window native boundaries automatically continue
the fixed plan without invoking a controller. Rejected/late commands do not close
or retarget a window; exact retries return the same receipt. Applied decisions,
including deadline fallbacks, replay without another historical controller call.

135 native tests (91 Simulator,33 Solver,11 policy) pass, including early service
with unchanged driving, prior-state observation isolation, waiting/duplicate/replay,
late/missing/out-of-range clocks and exact timeout fallback. The real JSON facade
marks one unaffected prefix and carries a deadline result. Strict library Clippy
and formatting pass. Browser/host window acceptance is tracked in game#316/#320;
these native tests alone do not establish continuous-play UI or full-GP acceptance.

## Opt-in observed-rival policy (12 November 2026)

`event_policy` selects `pitgun.racing-rival-pit-policy/v1` and requires
`service_windows` with an empty `decision_laps`. Omission preserves the scheduled
modes and their wire bytes. The complete policy is part of the pinned config/run
identity; each selected observation adds a frozen `trigger` with policy digest,
episode ID, observation horizon, own planned stop and exact source event IDs.

The filter consumes only released native event records, not rival plans, hidden
prefetched telemetry or final rankings. At each committed player-lap checkpoint:

- Consider newly observed rival service completions once. Compare that rival's
  same-lap crossing before service to the player's observed crossing of that lap.
  Missing crossings never imply proximity. This is a checkpoint gap, not an
  instantaneous gap, undercut predictor or optimal strategy recommendation.
- Require an eligible future service and an own stop that can be advanced by at
  least one lap, within the configured planning lookahead. No arbitrary wear or
  temperature alert thresholds are introduced.
- Group all eligible stops released since the previous review into one immutable
  episode. Sort by absolute pre-service crossing gap, occurrence time and fact ID.
  Deduplicate by fact ID. Input roster/pull/callback order does not change selection.
- Apply a completed-lap cooldown and run-wide window budget. Stops suppressed by
  stale/irrelevant/cooldown checks are consumed, not queued into a later stale
  notification. Events released during an active window do not amend its digest.

The game test fixture uses 8 s maximum pre-service crossing gap, 120 s maximum
observed-event age, own stop within 2 laps, at least 2 completed laps between
windows, and at most 3 windows per run. Configured closing margin remains 5 s.
These are explicit prototype thresholds, not calibrated GP/competitive defaults.
Polling remains at committed laps: the first relevant alert can therefore lag the
physical event by a lap. Episode grouping is checkpoint-based, not wall-time based.
This first case only offers an **earlier planned stop**, not reactive pit requests
for every possible race situation or a claim that stopping is beneficial.

The generic contract crate and published workloads are unchanged. Native tests
exercise grouped nine-rival events, silence, missing/far/future/old facts,
deduplication, cooldown, budget, frozen bindings, actual service changes, exact
Keep-plan output and recorded-action replay. Rejected/silent candidates are not
extra host decision checkpoints: they can be reconstructed from pinned runtime,
inputs, policy and the released event stream. Only selected episodes are carried
in the durable observation/proposal/receipt journal.

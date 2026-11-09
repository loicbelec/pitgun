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

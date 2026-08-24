# Racing deterministic driver instructions V1

Status: architecture contract for
[`loicbelec/pitgun#348`](https://github.com/loicbelec/pitgun/issues/348).
It defines the next runtime boundary but does not change a published model,
catalog, Solver equation, or game behavior by itself.

## Decision

Future timeline-enabled Racing sessions start every competitor in one common,
catalog-resolved driving mode. The first such Racing profile resolves
`balanced`.

`manage`, `balanced`, and `attack` are temporary deterministic instructions,
not permanent driver types. The player and AI competitors use the same event
contract, decision boundaries, validation rules, and physical consequences.
An empty instruction timeline keeps every competitor in the common default for
the complete session.

This decision follows the governed review of the static mode-response surface:
bounded coefficient tuning could not prevent `attack` from remaining the
universal winner when one mode was held for an entire race. Pitgun will model
mode changes and their persistent physical consequences before considering
further coefficient calibration.

## Ownership

| Layer | Owns | Does not own |
| --- | --- | --- |
| Catalog/model profile | supported modes, common default, physical response parameters, instruction limits | player identity, AI privilege, UI timing |
| Racing Simulator | deterministic boundaries, active-mode state, ordered transitions, session progression | mode physics, hidden pace bonuses |
| Racing Solver | physical response to the active resolved mode and persistent vehicle state | instruction scheduling, player or AI policy |
| Game / opponent policy | instruction requests and their presentation | alternate transition rules or physical meanings |
| Authority | initial run authorization and bounded instruction envelope | future clicks or AI decisions |
| Verifier | replay of the canonical applied history and final result | trust in browser animation or wall-clock timing |

The generic Pitgun framework remains unaware of Racing labels. It handles a
versioned workload and its canonical inputs, events, outputs, and evidence.

## Authoring and resolved contracts

The catalog-owned instruction profile fixes the common starting state and the
accepted event envelope:

```json
{
  "schema_version": "pitgun.racing-driver-instruction-profile/v1",
  "default_mode": "balanced",
  "boundary_granularity": "lap_start",
  "max_events_per_session": 64
}
```

The authoring contract carries only requested transitions:

```json
{
  "schema_version": "pitgun.racing-driver-instruction-timeline/v1",
  "events": [
    {
      "sequence": 0,
      "competitor_id": "player",
      "effective_at": {
        "lap_index": 3,
        "segment_index": 0
      },
      "mode": "attack"
    },
    {
      "sequence": 1,
      "competitor_id": "player",
      "effective_at": {
        "lap_index": 7,
        "segment_index": 0
      },
      "mode": "balanced"
    }
  ]
}
```

The resolved execution contract additionally records:

- the catalog and model identities that define the instruction surface;
- the common resolved initial mode;
- the resolved competitor identities;
- the resolved track segmentation used by every boundary;
- the canonical event list.

The author cannot provide a separate initial mode for the player or an AI
competitor. A later catalog may change the common default, but that change
produces a new immutable profile identity.

## Deterministic decision boundary

`lap_index` and `segment_index` are zero-based coordinates in the resolved
session and track. An event becomes active immediately before the Solver
evaluates the addressed segment.

This boundary is deliberately independent of:

- browser render frames;
- elapsed wall-clock time;
- network latency;
- telemetry batching;
- the time at which an animation happens to display the car.

The initial runtime may expose only lap-start boundaries by accepting
`segment_index = 0`. Supporting every governed segment later does not change
the event shape. A live request must address a boundary strictly after the
Simulator's last finalized boundary.

## Canonical ordering and validation

Before execution, events are ordered by:

1. `lap_index`;
2. `segment_index`;
3. `competitor_id` in bytewise lexical order.

`sequence` must be contiguous from zero and match that canonical order. This
makes the same accepted event set produce the same bytes regardless of UI or
transport ordering.

Validation fails closed when:

- a mode is not supported by the resolved profile;
- a competitor is absent from the resolved session;
- a boundary is outside the session or resolved track;
- an event targets a finalized or current live boundary;
- an event targets the initial `(0, 0)` boundary and would bypass the common
  default before the first physical step;
- two events target the same competitor at the same boundary;
- ordering or sequence numbers are non-canonical;
- the authorized event-count or boundary limits are exceeded.

An instruction that repeats the currently active mode is valid but redundant.
It remains in the canonical history because removing an accepted event would
make the evidence differ from the executed instruction stream.

## Runtime state

The Simulator owns one `active_mode` per competitor. At each boundary it:

1. finalizes the accepted event set for that boundary;
2. validates and canonically orders simultaneous events;
3. applies at most one transition per competitor;
4. resolves the active mode through the immutable model profile;
5. invokes the Solver for the next deterministic step;
6. emits progress, telemetry, and applied-instruction events;
7. carries tire, thermal, fuel, and later energy state into the next step.

Predeclared schedules are the first implementation. Live player requests and
deterministic AI decisions will enter the same acceptance path once the
incremental runtime in
[`#312`](https://github.com/loicbelec/pitgun/issues/312) exists.

The Solver boundary accepts an optional, strictly ordered schedule of resolved
physical controls at zero-based lap boundaries. Its scalar driver-control
fields remain the lap-zero baseline. The Solver never receives Racing mode
labels: the Simulator must translate each accepted instruction into bounded
utilization, control-error and correction-workload values before solving.

The first Simulator implementation performs that translation for an offline,
predeclared whole-grid timeline. It validates the canonical events once
against the resolved competitors, lap count and track, resolves the common
default independently for every driver, and then derives each competitor's
physical lap schedule through the existing driver-control equations. Player
and AI entries therefore traverse the same validation and resolution path.

An empty resolved schedule is omitted from the wire representation and retains
the existing static execution exactly. When a schedule is present, the result
records the baseline and every applied physical transition while preserving
continuous tire, thermal, fuel, speed, gear and shift state between laps.

## Identity and verification

Dynamic instructions separate attempt identity from final result identity:

- `execution_id` identifies the authorized execution attempt before future
  instructions are known;
- `run_id` identifies the completed canonical result and therefore commits to
  the exact applied instruction history.

Authority authorizes an instruction envelope containing at least:

- instruction schema identity;
- supported mode/profile identity;
- permitted decision-boundary granularity;
- maximum accepted event count;
- session and competitor scope.

Authority does not sign each player click or AI decision. Final evidence binds
the common initial mode and the canonical applied history. Verifier replays
that history and rejects any missing, reordered, inserted, or modified event.

The first identity slice is `RacingCompletedRunInputV1`. It layers the
following facts over the input identity signed before execution:

- the exact content-addressed instruction-profile identity;
- the common resolved initial mode;
- the complete, strictly sorted competitor set;
- the canonical applied timeline.

Its canonical digest becomes the `input.digest` of a derived final
`DeterministicRunContractV1`. Consequently, changing or removing one accepted
event changes the final `run_id`; non-canonical ordering, an unknown
competitor, a different profile identity, or an initial mode inconsistent with
the resolved profile fails closed.

This derived identity does not alter `RunAuthorizationV1`: that deployed
contract still authorizes an immutable run and expects its `run_id` before
execution. A later, separately versioned bounded authorization envelope must
link the stable `execution_id` to this completed identity. Until that envelope
and Verifier replay exist, timeline execution remains an offline candidate and
cannot be presented as Hosted Verification.

Late, duplicated, malformed, or unauthorized requests do not enter physical
execution or final evidence. They may be retained as operational diagnostics.
The browser outbox continues to preserve completed evidence when Hosted
Verification is temporarily unavailable.

## Compatibility

- Published catalogs and model identities retain their static whole-session
  mode behavior and exact replay semantics.
- A compatibility adapter may read their existing per-competitor static mode;
  it must not reinterpret those historical inputs as the new common default.
- Timeline-enabled profiles use the common catalog-resolved default and the
  new event contract. They do not accept hidden initial AI overrides.
- An empty timeline is the baseline for the new profile, not a rewrite of a
  historical run.
- Native Rust, WASM, Authority, and Verifier must share the same canonical
  fixtures before a timeline-enabled model is published.

## Delivery sequence

1. implement predeclared schedules in the Racing Simulator through
   [`#349`](https://github.com/loicbelec/pitgun/issues/349) (implemented first
   as an offline candidate; publication remains gated);
2. expose deterministic incremental progress through
   [`#312`](https://github.com/loicbelec/pitgun/issues/312);
3. bind the applied history into Hosted Verification through
   [`#350`](https://github.com/loicbelec/pitgun/issues/350);
4. let the live Pit Wall and AI policy issue events at the same boundaries;
5. measure race-distance mode usage and persistent costs locally and on
   Databricks;
6. adjust physical coefficients only when governed evidence justifies it.

## Non-goals

This contract does not:

- balance the three modes artificially;
- introduce random bonuses, rubber-banding, or hidden AI reactions;
- define the final Pit Wall interaction design;
- implement overtaking, traffic, qualifying, hybrid energy, or Pod Racing;
- require a network service to stream browser-local WASM progress.

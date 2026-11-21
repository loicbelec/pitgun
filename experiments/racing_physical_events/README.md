# Native derating events — bounded first increment

An opt-in Racing detector reads accepted native engine states, before graphical
resampling. It emits `started` / `recovered` into the existing session journal,
alongside `LapCompleted`, `PitStopCompleted` and `DriverInstructionApplied`.
No equation or catalog coefficient changes. No current game/runtime selects it.

## Reproduce

Commit the source first, then build both executables from that checkout:

```sh
cargo build --offline --locked --release -p pitgun-racing-simulator \
  --example native_derating_events_probe --example thermal_tire_gameplay_probe
python3 experiments/racing_physical_events/run_local.py \
  --probe target/release/examples/native_derating_events_probe \
  --legacy-probe target/release/examples/thermal_tire_gameplay_probe
```

Repeat the Python command with `--check`. Adapt executable paths when using an
external `CARGO_TARGET_DIR`. Source/binary/frozen input identities are recorded;
the second pass must reproduce the whole receipt and leaves its archive untouched.
No performance claim is made by this bank.

Each pass checks 22 frozen historical/energy outcomes (including expected errors),
then five frozen thermal-V2 cases under normal and pre-service execution. They
cover early cooling 0/2, modern cooling 0/4, and a 53-lap classic GP with a pit stop.
Both physical results and the full timed event journal must match exactly. The
second execution serializes/restores the detector, pending queue and journal at
driving/service boundaries; the Solver remains in memory. This does **not**
qualify a complete process/browser checkpoint, Authority capability or remote proof.
Only compact events/digests are archived, not full numerical trajectories.

## Meaning and limits

- A transition uses the native derating law (`factor < 1`), not a UI warning
  threshold. Persistent derating emits one start, not one message per sample/lap.
  Recovery links back to that episode. An active initial state is labelled as an
  initial observation, not an invented earlier crossing.
- Occurrence time is the first accepted native observation of the new state;
  microsecond transport precision does not imply root-localization accuracy.
  Post-service recovery is observed at its committed endpoint, not interpolated
  inside the abstract pit penalty. The terminal factor array repeats its previous
  segment, so terminal capability is evaluated from accepted terminal temperature.
- Numeric payloads use milli-Celsius and parts per million. Availability is not
  actual delivered shaft power. Rounding the payload does not select transitions.
- Repeated crossings are distinct native occurrences. An episode can stay open
  at race completion. More crossings do not establish greater thermal severity.
  A sufficiently cooled case can correctly produce no derating event.
- The native session explicitly selects player-aligned release; prefetched rival
  facts stay behind that frontier. This internal field journal is **not** the
  authorized player/model view: hidden rival temperatures require filtering.
- A versioned per-competitor audit bound fails closed on overflow; it never drops
  facts. A session whose physical-event journal fails cannot retry to completion
  while silently omitting events. This bound is not the future UI/LLM quota.

Detector state alone is insufficient for recovery: a host must bind its physical
context/policy and persist the ordered pending queue, emitted prefix and physical/
control checkpoint. Exact detection is independent of graphical export cadence
by construction; integration with the new V2 export is still unqualified.

Next: tire-window facts, deterministic relevance/grouping and capability-limited
observations, then explicit runtime/admission/checkpoint/Worker/Pit-Wall wiring.
Raw repeated threshold crossings are deliberately not UI alert episodes. Existing
decoders cannot consume the new variant without an updated opted-in contract.
Battery, fuel-DNF, incidents, VSC and weather require their own physical/race laws.

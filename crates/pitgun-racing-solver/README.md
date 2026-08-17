# pitgun-racing-solver

`pitgun-racing-solver` owns the deterministic physical and mathematical Solver
for the Racing domain.

It receives fully resolved vehicle, track, driver, tuning, state and pit-stop
inputs. It computes velocity, braking, acceleration, tire, thermal and energy
evolution, integrates the result through time, and can resample the physical
solution.

Every solve also derives `pitgun.racing-setup-response/v1` diagnostics from the
resolved physical track, spatial solution, selected gears, and actually applied
vehicle. These diagnostics explain a result without changing it: circuit
distance and curvature descriptors, straight/corner time and speed,
acceleration/braking time, shifts, observed RPM, aerodynamic drag work, and
downforce load. They are deterministic outputs, not tuning inputs or gameplay
bonuses.

The transformation from player-facing development points and setup sliders to
physical vehicle parameters is represented by
`pitgun.racing-tuning-response/v1`. `TuningResponseV1::default()` encodes the
historical Solver coefficients exactly; `apply_tuning` and `run_simulation`
remain compatibility entry points using that default. Explicit response APIs
exist for bounded offline calibration, but no player input or online endpoint
can select arbitrary coefficients. Candidate values must be validated by an
experiment before a future immutable Racing Catalog release adopts them.

It deliberately does not load catalogs, orchestrate races or sessions, produce
gateway envelopes, implement the linked workload, or expose browser bindings.
Those responsibilities belong to the Racing Simulator.

## Racing Game Model V3 candidate

`ResolvedSimulationRequestV3` is the first physical-only V3 boundary. It cannot
carry game development points or setup sliders, validates resolved physical
inputs, and integrates the real distance of every track segment. The candidate
is offline-only and documented in
[`RACING_MODEL_V3_FOUNDATION.md`](../../docs/RACING_MODEL_V3_FOUNDATION.md).

Historical V1/V2 entry points keep their original uniform-grid and tuning
behavior for deterministic replay.

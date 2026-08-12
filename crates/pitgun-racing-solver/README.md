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

It deliberately does not load catalogs, orchestrate races or sessions, produce
gateway envelopes, implement the linked workload, or expose browser bindings.
Those responsibilities belong to the Racing Simulator.

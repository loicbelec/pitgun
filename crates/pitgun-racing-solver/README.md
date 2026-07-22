# pitgun-racing-solver

`pitgun-racing-solver` owns the deterministic physical and mathematical Solver
for the Racing domain.

It receives fully resolved vehicle, track, driver, tuning, state and pit-stop
inputs. It computes velocity, braking, acceleration, tire, thermal and energy
evolution, integrates the result through time, and can resample the physical
solution.

It deliberately does not load catalogs, orchestrate races or sessions, produce
gateway envelopes, implement the linked workload, or expose browser bindings.
Those responsibilities belong to the Racing Simulator.

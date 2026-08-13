# Racing continuous curvature-response candidate V1

## Decision

A first continuous curvature response removes Spa's extreme-speed failure, but
it is not ready to replace the published Racing model. It remains accessible
only through a Rust offline experiment boundary; the game, WASM, catalog, and
hosted verification continue to execute the legacy model exactly.

## Why the binary response is problematic

The legacy Solver does not use one aerodynamic interpretation consistently:

- corner-speed limits and backward braking always use corner coefficients;
- forward acceleration switches abruptly at `0.001 rad/m`;
- diagnostics reproduce a separate binary classification.

Spa exposes the discontinuity because long, fast, weakly curved sections cross
that boundary repeatedly. This is a general Solver limitation, not a reason to
hard-code a Spa correction.

## Candidate function

The candidate maps absolute curvature to a blend from zero to one. It uses a
cubic smoothstep between `0 rad/m` (full straight coefficients) and
`0.001 rad/m` (full corner coefficients), then clamps at both ends. The
function is deterministic, continuous, bounded, symmetric in turn direction,
and monotonic.

Within the candidate execution, the same function supplies corner-speed
limits, backward braking, forward acceleration, and force diagnostics. Unit
tests protect those mathematical properties and prove that an explicit legacy
execution remains byte-for-byte equal to the normal production boundary.

An earlier broad transition ending at `0.005 rad/m` was rejected during local
exploration because it raised Spa's maximum speed to `410.78 km/h`. That
negative result is why the adopted experimental range stays close to the
legacy boundary.

## Seven-circuit evidence

The governed candidate replays the reviewed tuning response on an eleven by
eleven setup grid across Monza, Monaco, Budapest, Suzuka, Singapore,
Silverstone, and Spa: 847 deterministic runs.

| Circuit | Baseline optimum | Candidate optimum | Maximum-speed change |
|---|---:|---:|---:|
| Monza | 0.0 / 0.4 | 0.0 / 0.4 | -6.82 km/h |
| Monaco | 1.0 / 0.0 | 1.0 / 0.0 | -0.13 km/h |
| Budapest | 1.0 / 0.0 | 1.0 / 0.0 | -0.88 km/h |
| Suzuka | 0.3 / 0.0 | 0.2 / 0.1 | -5.20 km/h |
| Singapore | 1.0 / 0.0 | 1.0 / 0.0 | -3.09 km/h |
| Silverstone | 0.7 / 0.0 | 0.6 / 0.0 | -4.41 km/h |
| Spa | 0.0 / 0.9 | 0.0 / 0.8 | -10.54 km/h |

The global maximum falls from `406.39` to `395.86 km/h`, giving 4.14 km/h of
headroom under the holdout guardrail. Four of the five calibration optima are
unchanged. Suzuka moves by one grid step and falls just outside its historical
review interval; Silverstone and Spa also move by one gearing or downforce
step.

## Remaining blocker

The old aggregate `mean_corner_speed` invariant still reports eleven Spa
failures. That metric groups everything above one binary threshold, so using
it as the final judge of a continuous response would preserve the very
abstraction being removed. The candidate therefore does not pass the complete
governed grid yet, even though the speed guardrail passes and the five
calibration circuits introduce no new physical-invariant failures.

The next experiment must replace that legacy verdict with curvature-band
invariants shared with the Solver response, then review the one-step Suzuka
optimum movement. Only after that review may a separate PR propose a new model
identity and promotion into native/WASM execution.

The complete comparison is stored in
`experiments/racing_response/results/racing-continuous-curvature-response-v1.json`
with an adjacent SHA-256 checksum.

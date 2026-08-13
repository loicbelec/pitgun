# Racing curvature-band guardrails V1

## Decision

The continuous curvature-response candidate is physically eligible for a
separate, versioned model promotion. It passes the seven-circuit band-aware
guardrails and the 400 km/h maximum-speed guardrail. This evidence does not
activate it in the game, WASM, catalog, or hosted verification.

## Why the legacy verdict was replaced

The previous aggregate called every sample above `0.001 rad/m` a corner. At
Spa, maximum downforce improved genuinely tight curves while reducing speed in
medium curves, so the combined mean decreased and produced eleven apparent
failures. Requiring that aggregate to improve would preserve the same binary
abstraction the new response is intended to remove.

The V1 band-aware policy evaluates each of eleven gearing levels at minimum
and maximum downforce. Every comparison must show that maximum downforce:

1. reduces peak speed;
2. increases aerodynamic drag work;
3. increases mean downforce;
4. increases mean speed in the high-curvature band (`>= 0.005 rad/m`).

Near-straight, low-curvature, medium-curvature, and legacy mean-corner speeds
remain visible but informational. Losing speed in those bands can be the
expected cost side of the aerodynamic trade-off.

## Evidence

The campaign executes 154 deterministic simulations: seven circuits, two
downforce extremes, and eleven gearing levels.

| Circuit | Maximum speed | Band failures | Legacy failures |
|---|---:|---:|---:|
| Monza | 377.63 km/h | 0 | 0 |
| Monaco | 331.00 km/h | 0 | 0 |
| Budapest | 330.67 km/h | 0 | 0 |
| Suzuka | 375.72 km/h | 0 | 0 |
| Singapore | 348.42 km/h | 0 | 0 |
| Silverstone | 376.72 km/h | 0 | 0 |
| Spa | 395.86 km/h | 0 | 11 |

Across all circuits and gearing levels, high-downforce speed gains in the
high-curvature band remain positive. At Spa they range from 9.19 to
12.73 km/h, while the medium-curvature response ranges from -30.07 to
-25.71 km/h. The separation explains why the old combined metric failed and
why the candidate itself is physically coherent.

## Suzuka review

Suzuka's optimum moves from downforce/gearing `0.3 / 0.0` to `0.2 / 0.1`.
Both changes are exactly one step on the reviewed grid. Monza, Monaco,
Budapest, and Singapore preserve their established calibration optima. The
movement is therefore classified as bounded model sensitivity, not evidence
of a new discontinuity or a reason to retune coefficients before promotion.

## Promotion boundary

Promotion must remain a separate human-reviewed change because it alters the
deterministic output of native and WASM simulations. That change must:

1. assign a new Racing model identity;
2. make the continuous response the single production path;
3. remove the duplicated binary execution choice;
4. update native/WASM golden evidence together;
5. rebuild Authority and Verifier artifacts before staged game validation;
6. preserve the legacy model only where historical replay explicitly requires
   its old identity.

The complete evidence is stored in
`experiments/racing_response/results/racing-curvature-band-guardrails-v1.json`
with an adjacent SHA-256 checksum.

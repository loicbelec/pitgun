# Spa relief-response localization V1

## Decision

Spa's relief response is stable across three reasonable smoothing windows and
does not create a vertical-dynamics discontinuity. The profile is credible for
continued model research, but it is still not promoted into the catalog.

## Method

The experiment fixes setup at `0.25 / 0.25` and seed 42, then runs four
otherwise identical scenarios through the canonical Racing Simulator:

- a flat control;
- SPW relief smoothed over 75 metres;
- SPW relief smoothed over 125 metres;
- SPW relief smoothed over 175 metres.

All variants share the same one-metre `s/x/y` geometry, vehicle, governed
tuning response, and embedded catalog snapshot. Only `z` changes. A dedicated
offline Rust probe records distance, speed, throttle, brake, longitudinal
acceleration, and vertical acceleration. Python aligns those observations on a
25-metre grid and summarizes the most affected 250-metre sectors.

The probe is an analysis tool. It is not exported to WASM and is not part of
the game, hosted verification, or a production contract.

## Evidence

| Relief variant | Lap-time delta versus flat |
|---|---:|
| SPW 75 m | +200 ms |
| SPW 125 m | +203 ms |
| SPW 175 m | +203 ms |

The total spread is only 3 ms. The earlier conclusion therefore does not
depend materially on the selected smoothing window.

The largest absolute vertical response is 0.078 g for the least-smoothed
variant, far below the deliberately broad 3 g discontinuity guardrail. The
largest localized speed differences for that variant are -9.63 km/h at
4,725 m and +6.93 km/h at 3,550 m. Longitudinal response remains bounded,
while sharp brake-state differences mostly show that a small trajectory shift
moves a discrete braking transition to a neighboring sample.

The most persistent slow sectors lie between 1,000 and 2,250 m, where the
profile climbs. The strongest persistent gains lie around 500–750 m and
3,250–3,500 m, where it descends. This directional agreement is consistent
with the Solver's existing gravitational force term rather than a geometry or
serialization error.

## Interpretation

This closes two risks left by the first relief-impact experiment:

1. the measured lap-time effect survives wider smoothing choices;
2. deriving slope from the reviewed terrain profile does not generate an
   implausible vertical impulse.

It does not close the broader Spa high-speed problem. Relief changes the
physical solution, but the binary straight/corner segmentation and missing
energy model remain separate limitations. Adding elevation alone would not
make the current model production-complete.

## Next boundary

The next useful step is a governed design decision, not another elevation
source or an immediate catalog edit:

1. retain the validated SPW profile as experimental Spa evidence;
2. define how continuous curvature should replace binary segment labels;
3. test that change first against the existing flat multi-circuit guardrails;
4. replay Spa with and without relief to separate geometry and elevation
   effects;
5. publish a new immutable circuit resource only after those checks pass.

The complete aligned evidence is stored in
`experiments/racing_response/results/racing-spa-relief-localization-v1.json`
with an adjacent SHA-256 checksum.

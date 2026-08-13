# Spa relief impact V1

## Decision

The validated SPW terrain profile produces a material but bounded response in
the Racing Solver. It is credible enough to continue detailed review, but it
does not yet justify a catalog or production model change.

## Isolation

Both variants use the Simulator's explicit `track_profile` boundary with the
same 6,979 samples of `s`, `x`, and `y`. Consequently, both derive identical
heading and curvature. Only `z` changes: the control is flat, while the relief
variant interpolates the reviewed SPW profile onto the one-metre grid. The
Simulator derives slope deterministically from `s` and `z`.

The vehicle, governed tuning response, seed 42, and five-by-five setup grid are
identical. No catalog file, coefficient, game policy, WASM interface, or
production contract changes.

## Evidence

The 50-run local campaign reports:

| Metric | Flat | SPW relief |
|---|---:|---:|
| Fastest downforce / gearing | 0.25 / 0.25 | 0.25 / 0.25 |
| Fastest lap | 111.070 s | 111.270 s |
| Fastest-point maximum speed | 365.92 km/h | 359.66 km/h |

Across the 25 matched setups, relief adds between 177 and 378 ms, with a mean
of 245.64 ms. Mean maximum speed changes by -6.25 km/h. At the midpoint setup,
relief adds 222 ms and changes maximum speed by -6.19 km/h.

The optimum remains interior and unchanged on this coarse grid. Relief
therefore affects the physical solution without creating an implausible new
setup preference. Repeating the same midpoint execution produces exactly the
same output and experimental execution identity.

## Interpretation

The Solver already applies `mass × gravity × slope` to the longitudinal force
balance. Uphill and downhill sections therefore change acceleration and
braking even though a closed lap has no net elevation change. The observed
time penalty is not inherently suspicious: losses while climbing are not
perfectly recovered while descending because power, drag, grip, braking, speed
limits, and their positions around the lap are nonlinear.

## Boundary for the next step

Before considering a new immutable Spa resource:

1. inspect speed and longitudinal response by distance, especially around the
   largest SPW/EU-DEM differences;
2. review the 75-metre smoothing window and 14.71% maximum slope;
3. confirm no vertical-dynamics discontinuity or speed guardrail regression;
4. rerun the governed setup campaign only after those checks pass;
5. publish a new resource identity rather than modifying V1.1.0.

The complete evidence is stored in
`experiments/racing_response/results/racing-spa-relief-impact-v1.json` with an
adjacent SHA-256 checksum.

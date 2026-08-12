# Racing Spa high-speed diagnosis V1

## Decision

Spa exposes a general Solver segmentation weakness; it is not sufficient
evidence for changing the calibrated aerodynamic coefficients or patching one
circuit in isolation.

The V1 diagnosis therefore makes no catalog, gameplay, contract, or physical
model change. It adds a reproducible high-speed holdout guardrail and keeps the
five calibration optima selected by the mixed-circuit study unchanged.

## Reproduced observations

The diagnostic grid evaluates minimum and maximum downforce at eleven gearing
levels on Suzuka, Silverstone, and Spa: 66 deterministic one-lap runs.

- Spa reaches `406.392679991782 km/h`, reproducing the prior finding.
- The peak occurs around `2,093 m`, at absolute curvature
  `0.0009531359984763786 rad/m`.
- That curvature is just below the `0.001 rad/m` threshold, so forward
  acceleration selects the straight aerodynamic coefficients.
- All eleven Spa gearing levels reproduce the legacy
  `high_downforce_increases_corner_speed` failure.
- Silverstone and Suzuka remain below `400 km/h` and pass that aggregate legacy
  metric.

The report also confirms that the current response remains the same
calibration-eligible candidate selected across Monza, Monaco, Budapest, Suzuka,
and Singapore. Holdout observations cannot retroactively improve or replace
that calibration rank.

## Why the aggregate metric is misleading

The previous diagnostic calls every sample at or above `0.001 rad/m` a corner.
The curvature-band probe shows a more useful response:

- maximum downforce reduces mean speed in the near-straight, low-curvature,
  and medium-curvature bands on all three circuits;
- maximum downforce increases mean speed in the high-curvature band
  (`>= 0.005 rad/m`) at all eleven gearing levels on all three circuits;
- Spa contains enough medium-curvature response for the aggregate corner mean
  to become negative, while the other circuits' high-curvature gains hide the
  same underlying discontinuity.

The eleven Spa failures therefore do not mean that downforce never helps in
Spa's demanding corners. They mean that one binary aggregate mixes physically
different regimes.

## Solver and track findings

The same binary distinction is part of execution, not only reporting:

1. forward acceleration chooses straight or corner aerodynamic coefficients
   from the local curvature threshold;
2. corner-speed limits always use corner aerodynamics for every non-negligible
   curvature;
3. backward braking always uses corner aerodynamics;
4. the reporting path uses the threshold again, with a slightly different
   boundary convention from forward acceleration.

This creates a discontinuity around the threshold and inconsistent force
selection between Solver passes.

The physical circuit files also contain flat elevation and slope channels.
That is true for the comparison circuits as well, but it is especially visible
for Spa because the holdout archetype is named `mixed-elevation`. Elevation is
therefore absent from this experiment and cannot explain or validate Spa's
response yet.

## Guardrail

`racing-holdout-maximum-speed-v1` requires the maximum observed speed to remain
strictly below `400 km/h` over the bounded extreme-setup grid for Silverstone
and Spa. It is deliberately recorded as failing for the current model. The
guardrail blocks promotion claims; it does not make the existing CI red before
a corrective candidate exists.

## Next model work

The next bounded design should:

1. replace the duplicated binary aerodynamic mode with one shared,
   curvature-aware response used consistently by corner limits, braking,
   acceleration, and diagnostics;
2. retain curvature-band metrics so broad averages cannot hide a local
   regression;
3. audit the Racing track pack's curvature, elevation, and slope provenance;
4. replay the five calibration circuits and both holdouts before proposing any
   new catalog coefficients;
5. publish no automatic game or catalog change from this diagnosis alone.

## Reproduction

From the repository root:

```bash
cargo build --release -p pitgun-racing-simulator --example high_speed_response_probe
python3 experiments/racing_response/diagnose_spa_high_speed.py --jobs 4
python3 experiments/racing_response/diagnose_spa_high_speed.py --check --jobs 4
```

The canonical evidence is stored in
`experiments/racing_response/results/racing-spa-high-speed-diagnosis-v1.json`
with its adjacent SHA-256 checksum.

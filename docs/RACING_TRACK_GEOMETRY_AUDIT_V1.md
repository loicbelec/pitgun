# Racing track geometry audit V1

## Outcome

The Racing V1.1.0 pack contains 24 physical circuits with valid local XY and
curvature arrays, but every elevation and slope channel is a flat placeholder.
The immutable release remains unchanged.

The original geospatial lineage has been recovered:

1. WGS84 GeoJSON centerlines from `bacinger/f1-circuits` under the MIT License;
2. TrackEagle's equirectangular local projection around the mean coordinate;
3. smoothing and one-metre resampling;
4. import into Pitgun and later publication as immutable catalog resources.

All 24 source GeoJSON blobs match pinned upstream revision
`394d8fbe70ef2c0b0c8d23ff7bee61fa09606055` exactly. Their digests and the
current physical resource digests are stored in the audit report.

The audit also applies the versioned `pitgun.racing-track-validation/v1`
policy. It rejects missing, non-finite, or unequal-length channels; invalid
distance sampling; excessive closure gaps; geometry discontinuities; and
elevation/slope mismatches. Flat vertical channels remain valid historical
resources but carry an explicit `flat_vertical_placeholder` warning.

## Reversible coordinates

TrackEagle computes local coordinates as:

```text
x = radians(longitude - longitude₀) × R × cos(latitude₀)
y = radians(latitude - latitude₀) × R
```

where the reference is the arithmetic mean of the source coordinates and
`R = 6,371,000 m`. Retaining that reference makes the baked path reversible to
WGS84. For Spa, the recovered first point differs from the pinned GeoJSON by
less than one ten-millionth of a degree after smoothing.

## Spa EU-DEM prototype

The prototype samples the existing baked centerline every 25 metres, converts
those points back to WGS84, and queries EU-DEM v1.1 through OpenTopoData using
bilinear interpolation. The public API is used only to create a stored raw
artifact; simulation never depends on the service.

The 281-point prototype reports:

| Metric | Result |
|---|---:|
| Raw elevation | 364.71–471.96 m |
| Smoothed elevation range | 106.56 m |
| Smoothed cumulative gain/loss | 190.95 / 190.95 m |
| Maximum absolute slope | 0.2067 (20.67%) |
| Lap closure error | 0 m |

The range is plausible for Spa, but plausibility is not validation. The maximum
slope and accumulated gain remain sensitive to DEM accuracy and smoothing.
The prototype is therefore marked `experimental_not_catalog_eligible`.

## Slope semantics

The historical JSON field is named `slope_pct`, but TrackEagle calculates
`dz / ds` without multiplying by 100. Its actual unit is rise over run: `0.10`
means ten percent. A future schema should use an unambiguous name such as
`slope_ratio` or define the conversion explicitly before populated data enters
the Solver.

## Promotion boundary

Before a new catalog version can be proposed:

1. compare Spa against an independent or official elevation reference;
2. review smoothing and physically acceptable slope bounds;
3. resample the reviewed profile to the Solver grid;
4. quantify changes to speed, braking, lap time, and setup optima;
5. replay native/WASM determinism and all calibration/holdout campaigns;
6. publish new resource identities, digests, attribution, and methodology.

The source and EU-DEM licenses require attribution; details are retained in
`experiments/racing_tracks/NOTICE.md`.

# Racing Model V3 transmission resolution V1

Status: reviewed local candidate `pitgun.racing-v3-candidate@0.6.0`. It is not
published to a catalog and cannot be selected by the game, Authority or
Verifier.

## Problem

The preceding candidate multiplied every catalog gear ratio by an opaque
linear response:

```text
ratio multiplier = 1.10 - 0.20 * gearing slider
```

That response affected pace and produced circuit-specific optima, but did not
say what transmission state the player selected. It also made the catalog
coefficient difficult to review or extend toward hybrid energy management.

## Reduced-order transmission

Profile V4 gives the existing normalized slider one physical meaning. It
selects a target top speed between two versioned bounds:

```text
target speed = lerp(minimum target speed, maximum target speed, slider)
target rpm = maximum rpm * target engine-speed fraction
required top ratio = target rpm * 2π * wheel radius / (target speed * 60)
final-drive multiplier = required top ratio / catalog top-gear ratio
```

All quantities in the equation use SI units except engine speed in revolutions
per minute. Every catalog ratio receives the same final-drive multiplier, so
the relative spacing between gears is unchanged. Slider zero selects the
shortest transmission; slider one selects the longest.

The initial candidate range is `85..105 m/s` (306–378 km/h), reached at 98% of
maximum engine speed. These are screened parameters, not calibrated F1 truths.
Validation requires finite values, `40 <= minimum < maximum <= 150 m/s`, and a
target engine-speed fraction in `[0.5, 1]`.

The Solver still knows neither gameplay sliders nor catalog transport. It
receives the fully resolved gear ratios. Mechanical diagnostics expose the
resulting theoretical top speed at maximum RPM for the V4 profile only.

## Identity and replay

- profile V1 retains candidate `0.3.0`;
- profile V2 retains candidate `0.4.0`;
- profile V3 retains candidate `0.5.0`;
- profile V4 requires aerodynamic, development and transmission resources and
  selects candidate `0.6.0`;
- contradictory profile/resource combinations fail closed;
- the V1, V2 and V3 profiles and stored reports remain unchanged.

## Local evidence

The V4 campaign contains 792 deterministic three-lap executions: three seeds,
three representative circuits, twenty-five isolated physical controls, a
constant forty-point development budget and a five-by-five setup grid.

Changing the gameplay gearing slider from `0.2` to `0.8` produced:

| Circuit | Time effect | Observed maximum-speed effect | Theoretical top-speed effect |
| --- | ---: | ---: | ---: |
| Monza | -233 ms | +10.57 km/h | +44.08 km/h |
| Monaco | +237 ms | +4.51 km/h | +44.08 km/h |
| Suzuka | -768 ms | +10.65 km/h | +44.08 km/h |

Negative time is faster. A longer transmission helps the power and mixed
fixtures in this screen but hurts Monaco. The coarse combined setup grid also
retains three distinct optima:

| Circuit | Reviewed optimum (downforce / gearing) |
| --- | --- |
| Monza | `0.50 / 0.75` |
| Monaco | `1.00 / 0.00` |
| Suzuka | `1.00 / 0.75` |

All three transmission parameters are active in governed observables and show
circuit-dependent pace directions. Physical activation and circuit-dependent
setup therefore remain `PASS`; development specialization and production use
remain `REFINE`.

## Limits and decision

Accept the explicit final-drive interpretation and candidate identity. Do not
promote the initial bounds to the game or regenerate opponents yet.

This reduced-order slice does not model independent per-gear tuning, a
differential, clutch behavior, traction control or hybrid deployment. Those
features must be introduced only when their state and decisions are explicit,
especially when staged energy management is added under #246.

Next, extend the governed audit to held-out circuits, longer thermal windows
and active-era vehicles, then replay the accepted candidate through Delta and
MLflow on Databricks before any production decision.

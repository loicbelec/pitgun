# Racing Model V3 aerodynamic efficiency V1

Status: reviewed local candidate `pitgun.racing-v3-candidate@0.4.0`. It is not
published to a catalog and cannot be selected by the game, Authority or
Verifier.

## Problem found by the 0.3.0 screen

The first V3 decision-surface campaign showed that maximum downforce and short
gearing won on Monza, Monaco and Suzuka. The transitional V2 resolver increased
downforce more rapidly than drag and made aerodynamic development multiply both
areas together. It therefore provided no explicit aerodynamic-efficiency gain.

Changing AI profiles could not repair this surface: every opponent would still
be searching the same universally dominant direction.

## V3 resolver

The Simulator now resolves six explicit coefficients before calling the Solver:

- minimum and maximum downforce-area multipliers;
- base drag-area multiplier;
- induced-drag factor in `1/m²`;
- downforce-development gain at the axis cap;
- drag-development reduction at the axis cap.

For setup position `s` and normalized aerodynamic development `p`:

```text
ClA_setup = ClA_reference * lerp(ClA_min, ClA_max, s)
ClA = ClA_setup * (1 + downforce_development_gain * p)

added_ClA = ClA_setup - ClA_reference * ClA_min
CdA = (CdA_reference * base_drag_multiplier
       + induced_drag_factor * added_ClA²)
      * (1 - drag_development_reduction * p)
```

This is a declared reduced-order aerodynamic polar, not CFD. It captures the
important first-order trade-off: asking for more downforce increasingly costs
drag, while development improves efficiency. The Solver accepts only the final
fixed areas and remains independent of gameplay controls.

## Identity and replay

- experiment profile V1 still selects the immutable mechanical candidate
  `0.3.0` and its transitional aero mapping;
- profile V2 requires the new aero-resolution resource and selects `0.4.0`;
- malformed version/resource combinations fail closed;
- the previous V1 report and profile remain checked in unchanged.

## Local evidence

The V2 campaign executes 648 deterministic three-lap simulations using three
seeds, three circuit archetypes, seventeen isolated physical controls, a
constant development budget and a five-by-five downforce/gearing grid.

| Circuit | Reviewed optimum (downforce / gearing) |
| --- | --- |
| Monza | `0.50 / 0.25` |
| Monaco | `1.00 / 0.25` |
| Suzuka | `0.75 / 0.50` |

The circuit-dependent setup gate changes from
`STRUCTURAL_CHANGE_REQUIRED` to `PASS`. No circuit identifier or archetype is
used by the equation; the different optima emerge from the physical traces.

Aerodynamic development improves `ClA/CdA` in the isolated resolver test, but
it does not yet win the fixed-budget gameplay simplex because chassis remains
over-dominant. Development specialization therefore remains
`STRUCTURAL_CHANGE_REQUIRED` and is the next model correction.

## Decision

Accept the aerodynamic equation as the next offline V3 candidate slice. Do not
promote it to the game or generate opponent profiles yet.

Next:

1. separate chassis mechanical development from the aggregate tire-force
   budget and restore useful fixed-budget specialization;
2. review gearing on the resulting surface;
3. add held-out circuits and active-era vehicles;
4. replay the exact accepted candidate through Delta and MLflow on Databricks.

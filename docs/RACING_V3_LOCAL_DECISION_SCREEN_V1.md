# Racing Model V3 local decision screen V1

Status: reviewed local diagnostic for `pitgun.racing-v3-candidate@0.3.0`.
This is the first evidence slice of #244, not a production calibration or a
published Racing model.

## Question

Model V3 now exposes physically named tire, mechanical and driver controls.
Before spending Databricks compute or designing a new opponent policy, Pitgun
must distinguish two failure classes:

- an inactive coefficient that never reaches the result;
- an active equation whose decision surface still creates one universal setup.

The second case cannot be fixed responsibly by generating more AI profiles.
It requires a better trade-off in the model or its gameplay-to-physics resolver.

## Reproducible protocol

The committed campaign runs 351 deterministic points:

- physical archetypes: Monza, Monaco and Suzuka;
- seeds: `7`, `42`, `99`;
- three laps per execution;
- exact candidate identity `pitgun.racing-v3-candidate@0.3.0`;
- one versioned V3 profile and one immutable release probe;
- constant forty-point budget when comparing development allocations;
- isolated low/high values for eleven physical controls;
- low/high downforce and gearing plus a two-by-two interaction grid.

The canonical report records the runner digest, base input digests, every
execution identity, all result points, compact pair summaries and explicit
verdicts. No live resource is read or changed.

## Result

### Physical controls: `PASS` for activation

Every screened physical control changes at least one representative circuit by
more than 5 ms. Braking force, shift duration, driveline efficiency, tire load
sensitivity, tire heat/cooling/wear, the three driver utilization channels and
driver error are therefore connected to the V3 result.

This does **not** mean their current values are calibrated. The thermal response
is especially informative: more tire heat is faster and more tire cooling is
slower throughout the reviewed range. The coefficients are active, but the
operating window needs calibration and longer-run evidence.

### Circuit-dependent setup: `STRUCTURAL_CHANGE_REQUIRED`

The same coarse setup wins on all three circuit archetypes:

| Circuit | Fastest reviewed setup |
| --- | --- |
| Monza | downforce `0.8`, gearing `0.2` |
| Monaco | downforce `0.8`, gearing `0.2` |
| Suzuka | downforce `0.8`, gearing `0.2` |

High downforce is faster despite reducing maximum speed. Short gearing is also
faster everywhere. A denser grid may locate a nearby numerical optimum, but it
cannot overturn the observed absence of archetype-specific direction in this
range. The next model slice must create an actual lap-time cost for excessive
downforce and gearing choices rather than merely selecting different AI presets.

### Development specialization: `STRUCTURAL_CHANGE_REQUIRED`

At the same total development budget, transferring points toward chassis is
faster on every reviewed circuit. Transferring points toward engine, cooling,
or aero is slower on every reviewed circuit because the points are taken away
from the dominant chassis axis.

This explains why a player can ignore several controls and still win. The
opponent generator is not the root cause: it is searching a surface with a
nearly universal direction.

## Decision

Do not calibrate or publish an opponent policy from this surface. Do not change
the production catalog or Model V2.

The next V3 work should target the structural trade-offs exposed here:

1. make fixed `CdA` and `ClA` resolution produce circuit-dependent aero cost;
2. review gear-ratio resolution against the sequential transmission and usable
   engine-speed range;
3. separate chassis mechanical grip gains from the aggregate tire load budget
   so chassis cannot dominate the complete fixed-budget simplex;
4. calibrate tire and engine thermal operating windows on longer runs;
5. replay the corrected exact candidate on held-out circuits and active eras,
   then freeze the identical campaign for Delta/MLflow on Databricks.

Only after those gates pass should Pitgun generate opponent profiles and pit
strategies from the reviewed decision surface.


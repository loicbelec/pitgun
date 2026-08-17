# Racing Model Constitution V1

Status: governed foundation for issue #242. This document describes what the
Racing model is allowed to claim, where its inputs belong, and how it may gain
fidelity without losing deterministic replay.

## Purpose

The Racing model is a deterministic engineering model for comparing vehicles,
setups, strategies, and race evolution. It aims to become progressively more
physically meaningful and explainable. It is not currently a high-fidelity,
validated Formula 1 vehicle model, and gameplay balance is not evidence of
physical accuracy.

The model must remain useful in four different contexts:

- an executable example of the generic Pitgun simulation loop;
- the physical foundation of the Pitgun game;
- a governed source of reproducible telemetry and experiment datasets;
- a learning platform whose assumptions and limitations can be inspected.

## Ownership boundary

The Solver / Simulator separation is a permanent architectural constraint.

The Racing Solver owns resolved physical types, equations, numerical methods,
state evolution, and physical diagnostics. It does not know where catalog
resources came from, which game era is active, how opponents are selected, or
whether a result will be submitted to a leaderboard.

The Racing Simulator owns catalog resolution, vehicle and strategy selection,
race and session orchestration, telemetry projection, evidence construction,
and the linked workload exposed to the Pitgun runtime. It invokes the Solver;
it must not duplicate physical equations.

The game owns progression, economy, player interaction, and presentation. An
opponent policy may choose valid model inputs, but it must not introduce a
hidden pace multiplier that masks a weak physical decision surface.

## Five kinds of model information

| Kind | Examples | Owner | Change mechanism |
| --- | --- | --- | --- |
| Equations and algorithms | force balance, speed-profile passes, time integration, thermal evolution | versioned Rust Solver workload | new model identity |
| Physical resource data | circuit samples, torque curves, mass, drag area, tire curves | immutable Racing Catalog Simulation Pack | new catalog release |
| Calibrable coefficients | bounded setup and development response, future energy losses | immutable model resource in the catalog | new catalog release, compatible model contract required |
| Gameplay policy | opponent styles, budgets, setup and strategy sampling | immutable Racing policy resource | new catalog release |
| Execution input | selected vehicle, tuning, driver, tire, strategy, seed | deterministic run contract | new execution and `run_id` |

A coefficient belongs in the catalog only when its physical meaning, unit,
bounds, provenance, and compatibility are documented. Moving arbitrary
constants out of Rust is not, by itself, a better model.

## Current physical system

The current Solver resolves a vehicle from chassis, aerodynamic, combustion
engine, and tire parameters. Its evolving state is:

- fuel mass in kilograms;
- normalized tire wear;
- tire temperature in degrees Celsius;
- engine temperature in degrees Celsius.

The resolved track provides distance, planar position, elevation, curvature,
slope, and heading samples. Coverage and source quality can differ between
circuits; non-zero elevation arrays do not by themselves establish surveyed
track accuracy.

The principal controls are four development allocations, downforce and gearbox
sliders, driver choice, tire and pit-stop strategy, and the deterministic seed.
The principal outputs are a spatial and temporal velocity solution, lap and
race times, state evolution, resampled telemetry, and setup-response
diagnostics.

At a high level, one lap currently consists of:

1. a lateral-grip corner-speed limit over the spatial track;
2. a backward braking pass constrained by grip, drag, rolling resistance,
   slope, and normal load;
3. a forward acceleration pass constrained by the engine curve, gearbox,
   traction, aerodynamic forces, slope, and thermal derating;
4. time integration of the final speed profile;
5. engine-temperature, tire-temperature, tire-wear, and fuel-mass evolution;
6. deterministic driver pace adjustment and pit-stop state transitions.

Racing model V2 applies a continuous curvature response to aerodynamic forces.
It does not yet change the historical development and setup response encoded by
`TuningResponseV1::default()`.

## Current shortcuts and known gaps

The following limitations are part of the model contract until replaced by a
new governed identity or resource:

- development response is a small compiled set of linear coefficients;
- each development axis is independently clamped at 20 Solver points;
- chassis development directly scales the base tire-grip coefficient;
- engine development scales the complete torque curve uniformly;
- aero development scales drag and downforce together;
- cooling has no lap-time value unless the engine enters thermal derating;
- gearbox tuning scales every ratio together rather than designing a gear set;
- fuel burn is time based and fuel energy is not balanced explicitly;
- there is no battery state of charge, electrical power limit, recovery,
  conversion loss, reserve target, or deployment controller;
- a resource named as hybrid or active-aero does not mean those capabilities
  are physically simulated;
- interactions, saturation, and progression across eras have not yet passed a
  complete decision-surface audit.

The early-allocation experiment gives direct evidence of these gaps: at its
reviewed four-point boundary, chassis was strongly dominant, engine was weak,
cooling was inactive, and aero was slightly harmful. Those results must guide
the audit; they must not be copied into a stronger opponent policy.

## Capability identifiers

Downstream specifications and tests use the following stable identifiers. A
capability can be `implemented`, `simplified`, `planned`, or `absent` for a
given era.

| Capability ID | Meaning |
| --- | --- |
| `racing.vehicle.longitudinal` | longitudinal force balance, acceleration, braking, and speed integration |
| `racing.track.curvature` | curvature-dependent lateral and aerodynamic response |
| `racing.track.elevation` | slope and vertical-profile forces from catalog track data |
| `racing.aero.fixed` | fixed drag and downforce response |
| `racing.aero.active` | commanded, stateful aerodynamic configuration |
| `racing.powertrain.ice` | combustion-engine torque curve, gearbox, fuel mass, and thermal derating |
| `racing.tire.state` | tire temperature, wear, compound, and load-sensitive grip |
| `racing.energy.accounting` | explicit source, storage, loss, recovery, and delivered-energy balance |
| `racing.energy.automatic-control` | deterministic model-owned energy deployment |
| `racing.energy.strategic-control` | bounded player- or agent-owned energy decisions |
| `racing.mission.fixed-path` | ordered mission segments whose geometry is already resolved |
| `racing.propulsion.lift` | energy required to create and control non-ground-supported lift |

The authoritative per-era status is recorded in
[Racing Era Capability Matrix V1](RACING_ERA_CAPABILITY_MATRIX_V1.md).
The equation-level limitations, fidelity boundary, and proposed V3 sequence are
recorded in the
[Racing Model Approximation Audit V1](RACING_MODEL_APPROXIMATION_AUDIT_V1.md).

## Physical and numerical invariants

Every production model must preserve these invariants:

- identical resolved inputs and identities produce identical canonical output;
- all runtime physical values are finite and use documented units;
- mass, speed, elapsed time, temperatures, and available energy cannot become
  negative merely to complete a run;
- a pit stop changes only the states and elapsed time declared by its contract;
- telemetry is a projection of the solved state, not an independent source of
  race performance;
- an energy-capable model balances initial energy, supplied energy, recovered
  energy, delivered work, modeled losses, and final stored energy within a
  declared numerical tolerance;
- a controller cannot deploy energy that the resolved storage does not contain
  or violate a declared reserve without producing a deterministic failure;
- historical model and catalog identities remain replayable and are never
  rewritten in place.

Golden native and WASM executions protect deterministic equivalence. Authority
and Verifier must authorize the same model digest, catalog Simulation Pack,
contract version, and coefficient resource identity used by the browser.

## Progressive-fidelity rule

New physics enters the production model only when it has:

1. a named capability and a user or engineering question it answers;
2. explicit state, controls, units, parameters, and validity domain;
3. invariants and deterministic native/WASM tests;
4. sensitivity evidence showing that its controls are useful but not
   universally dominant;
5. a catalog and model compatibility rule;
6. replay, Authority, Verifier, rollout, and rollback coverage.

This rule intentionally rejects both extremes: freezing an obviously weak
model for compatibility, and adding maximum complexity before its value can be
measured.

## Versioning decisions

Changing an equation, state transition, numerical method, capability meaning,
or canonical output requires a new Racing model identity. Adding a new energy
state therefore cannot silently modify `pitgun.racing@2.0.0`.

Changing validated resource values or a bounded coefficient resource creates a
new immutable catalog release. Compatibility metadata must state which model
contract can interpret it. A mutable pointer may select a release, but it never
changes the release bytes.

Changing an opponent distribution creates a new policy resource and catalog
release. It is not a model calibration unless physical resources or
coefficients also change.

Changing a player's setup, strategy, or seed creates a new execution. It does
not create a new model or catalog version.

## Audit gates before Opponent Policy V3

The next multi-era decision-surface campaign must demonstrate that:

- every player control has a measurable activation domain or is removed from
  the active era;
- no unexplained allocation is universally dominant across representative
  circuits, budgets, and strategies;
- useful setup optima differ between circuit classes;
- combined controls expose interactions rather than only independent linear
  bonuses;
- progression remains effective beyond the current per-axis clamp;
- held-out circuits preserve the reviewed conclusions;
- physical diagnostics explain the direction of each material effect.

Only after those gates are reviewed should a new AI policy sample competitive
frontiers from the model.

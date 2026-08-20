# Racing Model Approximation Audit V1

Status: governed architecture decision for issue #251. This audit changes no
production equation, catalog resource, game policy, or deterministic identity.
It defines what may enter a future Racing Game Model V3 and what must remain a
claim of the historical V1/V2 models only.

## Decision

Pitgun will not pursue one model with an undocumented mixture of gameplay
shortcuts and increasingly detailed physics. It will grow two explicit model
families:

- the **Game Model** is a deterministic reduced-order model designed for native
  and browser/WASM execution;
- the future **Reference Model** is a progressively richer offline engineering
  model intended for studies, calibration, and a public model experience on
  `pitgun.com`.

The Game Model remains physical. An approximation is allowed only when its
state, equation, units, validity domain, limitations, and upgrade path are
documented. The Reference Model is not a hidden high-difficulty game mode and
the Game Model is not a runtime flag that bypasses arbitrary equations.

Historical `pitgun.racing@1.0.0` and `pitgun.racing@2.0.0` executions remain
immutable and replayable. The findings below are inputs to a new model
identity; they do not reinterpret historical results.

## Audit method and verdicts

The audit reviews the current Rust equations, catalog resolution, race
orchestration, deterministic tests, and governed experiment evidence. It uses
three verdicts:

- `RETAIN`: the equation or boundary is suitable for the declared Game Model
  validity domain;
- `REFINE`: the structure is useful but its semantics, coupling, validation,
  or parameterization must improve;
- `REPLACE`: retaining the structure would preserve a misleading or internally
  inconsistent physical claim.

A verdict is not a request for maximum complexity. `REPLACE` may select a
better reduced-order equation rather than a high-fidelity implementation.

## Honest validity claim

### Racing Model V2

The current model is suitable for:

- deterministic comparison inside one pinned model and catalog identity;
- reproducible setup, strategy, telemetry, and data-engineering experiments;
- directional studies whose exact controls have passed a governed campaign;
- the existing game's historical replays and Hosted Verification evidence.

It is not currently evidence for:

- absolute Formula 1 vehicle or lap-time accuracy;
- wheel-level tire, suspension, braking, or aerodynamic behavior;
- fuel or electrical energy management;
- transient gearshift, launch, or drivetrain performance;
- weather, track evolution, traffic, overtaking, or race incidents;
- physical driver skill inferred from a post-solve time adjustment.

### Target Game Model V3

V3 should claim a deterministic, dry-condition, reduced-order race model with
coherent point-mass forces, aggregate transient tires, resolved physical
vehicle parameters, and era-gated modules. It may continue to simulate
competitors independently before wheel-to-wheel interaction is implemented,
but that limitation must be visible in its identity and documentation.

### Future Reference Model

The Reference Model may add axle or wheel resolution, richer thermal and tire
states, efficiency maps, detailed energy losses, weather, and operational-data
calibration. It has a separate workload identity and validation suite. A result
from it is never represented as if it came from the Game Model.

## Executive findings

| Priority | Finding | Consequence |
| --- | --- | --- |
| P0 | Gameplay development points enter the Solver and directly mutate physical parameters. | Economy and physics ownership are mixed; V3 must receive a physically resolved vehicle. |
| P0 | Tire state is coupled inconsistently: lateral and braking limits use lap-start state, forward traction uses evolving state, and acceleration does not reserve lateral grip. | Tire degradation and chassis dominance cannot be fixed reliably through coefficient sweeps alone. |
| P0 | Fuel is burned by elapsed time with no power or energy balance, and the hybrid resource has no electrical state. | A new explicit energy boundary is required before hybrid gameplay can be claimed. |
| P1 | Aerodynamic coefficients are selected from track curvature rather than a physical or commanded vehicle state. | Continuous curvature removed a discontinuity but remains a reduced heuristic, not passive or active aero physics. |
| P1 | Gear choice instantaneously selects maximum available power with no shift thresholds, duration, loss, or driveline efficiency. | Gearbox setup has limited and partially artificial authority. |
| P1 | Driver pace is applied after the physical solve by rescaling time while leaving the solved speed and state trajectory unchanged. | Driver effects can make clock, state evolution, and telemetry mutually inconsistent. |
| P1 | The spatial algorithm assumes effectively uniform spacing and carries a one-metre-sensitive vertical-load expression. | Arbitrary valid `s` grids do not have a fully documented numerical meaning. |
| P1 | Catalog identity validation is strong, but physical plausibility validation is weak and several units are implicit. | Immutable invalid physics can still be faithfully resolved and replayed. |
| P2 | Race positions are rankings of independent clean-air simulations with a flat pit-time addition. | This is a valid first game approximation only when explicitly described as such. |

## Architectural boundary

### Physical resolution before the Solver

The game may expose points, upgrades, doctrines, eras, and named parts. These
are not physical quantities. For V3:

```text
game progression + catalog model resource + selected setup
                              |
                              v
                 resolved physical vehicle
                              |
                              v
                    versioned Racing Solver
```

The resolver belongs to the Racing Simulator/catalog boundary. Its immutable
resource maps gameplay decisions to named physical parameter changes. The
Solver receives mass, force coefficients, curves, limits, states, and physical
controls. It must not know the price, era unlock level, or point balance that
produced them.

`TuningResponseV1` remains part of historical V1/V2 compatibility. Issue #245
must not simply move its current fields into a new production resource and call
that V3. It should first separate:

- gameplay-to-physical resolution;
- physical setup controls;
- model equation coefficients;
- numerical safety parameters.

### Model identities, not a fidelity switch

The recommended identity rule is:

- retain `pitgun.racing@<version>` for the verified Game Model lineage;
- introduce a distinct future model ID such as
  `pitgun.racing-reference@<version>` for the Reference Model;
- bind each execution to the exact model digest and compatible Simulation Pack;
- never accept a player-supplied `fidelity=reference` switch inside one signed
  model identity.

Shared catalog concepts are allowed, but compatibility is explicit for each
model. A parameter resource supported by one family is not automatically valid
for the other.

## Subsystem summary

| Subsystem | Current Game Model approximation | Verdict for V3 | First V3 objective |
| --- | --- | --- | --- |
| point-mass spatial solve | backward braking plus forward acceleration envelope | `REFINE` | preserve deterministic architecture; use explicit segment lengths and consistent state coupling |
| chassis and normal load | single mass, grip scalar, rolling resistance, no axle/load sensitivity | `REFINE` | add aggregate load sensitivity and named braking limits without suspension simulation |
| tires | one wear and temperature state with acceleration-squared workload | `REPLACE` equations, `RETAIN` aggregate state idea | consistent aggregate force utilization, temperature, and wear across the complete lap |
| fixed aerodynamics | `q × effective area`, with curvature-driven state interpolation | `RETAIN` force law, `REPLACE` implicit state selection | fixed coefficients for passive aero; explicit commanded state for future active aero |
| combustion engine | torque curve, instantaneous power, first-order thermal derating | `REFINE` | document units, power-dependent fuel flow, calibrated lumped thermal state |
| transmission | maximum-power gear chosen at every sample | `REPLACE` | sequential gear state, shift rule, duration/loss, driveline efficiency |
| fuel and energy | lap-constant mass; time-based burn; no balance | `REPLACE` | power-based fuel accounting and modular battery/power-bus state |
| driver | tire-wear multiplier plus post-solve pace/noise milliseconds | `REPLACE` physical pace transform | bounded physical-limit utilization and deterministic control error |
| environment | air density and gravity stored in chassis; fixed ambient temperature | `REFINE` | event environment resource with dry baseline |
| track | planar path plus optional relief; flat production vertical channels | `REFINE` | explicit slope units, uniform-grid contract or per-segment integration, reviewed relief resources |
| pit and race evolution | tire reset plus flat loss; competitors solved independently | `RETAIN` as declared V3 scope, then `REFINE` | circuit-specific pit loss and clean-air race-profile identity |

## Detailed equation audit

### 1. Spatial solution and force balance

The current model constructs a corner-speed envelope, propagates braking
backward, propagates acceleration forward, and integrates the minimum speed
profile over distance. This architecture is fast, deterministic, explainable,
and suitable for a reduced-order Game Model. It is therefore retained.

The longitudinal terms are currently, in simplified notation:

```text
drag       = 0.5 × rho × v² × CdA
downforce  = 0.5 × rho × v² × ClA
rolling    = Crr × normal_load
slope      = mass × g × slope_ratio
```

The main limitations are structural rather than cosmetic:

- one `ds`, derived from the first two track samples, is reused for every
  segment even though validation only requires `s` to increase;
- tire, thermal, and mass state are not solved with one consistent coupling
  contract across the backward and forward passes;
- the forward pass limits drive force by all available `mu × normal_load`
  even when lateral force is already required in a corner;
- the backward pass uses a circular friction budget, making acceleration and
  braking asymmetrical;
- braking is tire-limited with a compiled `6 g` cap but has no brake-system,
  balance, or thermal limit;
- low-speed engine force uses `max(v, 10 m/s)` as a numerical regularization
  that also shapes launch performance.

V3 should preserve the envelope method but use per-segment distance, the same
combined-utilization rule in acceleration and braking, and a deterministic
fixed-point or predictor/corrector contract for transient state. Convergence
criteria and failure behavior must be outputs of the numerical specification,
not accidental loop counts.

The Reference Model may later split front/rear axle loads or wheels. V3 does
not need suspension kinematics to correct its current combined-force
inconsistency.

### 2. Track, relief, and environment

Track curvature and slope alter the force balance, and the continuous V2
curvature function is deterministic. The production catalog still publishes
flat `z` and slope placeholders for all named circuits. Reviewed Spa relief is
experimental evidence, not production data.

The field `slope_pct` actually carries a rise-over-run ratio: `0.10` means ten
percent, not the number ten. V3 must rename or strictly document that unit.

Vertical acceleration is derived from a sample-index slope difference and then
divided by `ds²`. For a uniform grid, the intended vertical curvature term is
proportional to the derivative of slope with distance. The current expression
is consequently sensitive to the hidden one-metre grid assumption; it must be
re-derived and tested dimensionally before non-unit grids or relief are
published for V3.

Air density and gravity currently live in the chassis, while engine and tire
ambient temperatures are duplicated elsewhere. V3 should resolve a dry event
environment containing at least gravity, air density, air temperature, and
track temperature. Weather remains a later Simulator capability.

### 3. Chassis, grip, braking, and load

The chassis is an aggregate point mass with wheel radius, base `mu0`, rolling
resistance, air density, and gravity. This is an acceptable Game Model starting
point, but `mu0` currently performs too many jobs:

- chassis development scales it directly;
- tire compound scales it;
- temperature and wear scale it;
- it has no sensitivity to vertical load;
- the same value governs lateral traction, acceleration, and braking.

Constant friction coefficient makes aerodynamic load convert too efficiently
into cornering force and is one plausible structural contributor to dominant
downforce/chassis responses. V3 should add an aggregate, bounded load-
sensitivity law referenced to a documented normal load. A later Reference
Model can distribute load across axles and account for weight transfer,
suspension, camber, and pressure.

Brake capacity should become a named resolved vehicle limit. Regenerative
braking remains absent until the energy module is active. The Game Model may
retain one aggregate friction ellipse and one brake limit rather than simulate
hydraulics or individual discs.

### 4. Tire temperature, degradation, and grip

The current grip equation is:

```text
wear_factor = max(1 - wear_grip_k × wear, wear_min)
temp_factor = max(exp(-((temp - temp_opt) / temp_sigma)²), temp_min_k)
mu_effective = chassis_mu0 × compound_mu_scale × wear_factor × temp_factor
```

Temperature and wear evolve as:

```text
load_metric = lateral_acceleration² + longitudinal_acceleration²
temperature_rate = heat_k × load_metric
                 - cool_k × speed × (temperature - ambient)
wear_rate = wear_per_s + wear_load_k × load_metric
```

This is deterministic and produces an interpretable aggregate state, but its
`load_metric` has squared-acceleration units and is not tire force, frictional
work, slip energy, or a documented empirical proxy. Its coefficients therefore
have implicit compound units.

More importantly, coupling is inconsistent:

- the corner-speed envelope uses tire wear and temperature from the start of
  the lap at every location;
- the backward braking pass also uses that lap-start state;
- the forward pass updates tire state along distance and uses it for traction;
- lateral demand is not subtracted from forward traction capacity;
- the resulting transient tire arrays do not feed the lateral envelope until
  the next lap.

Model V3 candidate `0.2.0` replaces this behavior on its isolated execution
path. V1/V2 retain it solely for replay compatibility. The candidate uses one
load-sensitive combined force budget, one shared state trajectory refined by
four fixed iterations, and a named contact-workload energy proxy for heat and
wear. Its coefficients remain pre-calibration candidate inputs rather than
published catalog truth.

The audit assigns `REPLACE` to the equations, not to the idea of one aggregate
tire state. The first V3 tire model should remain approachable:

1. retain one aggregate wear state and one aggregate temperature state;
2. derive a bounded combined tire-utilization metric from longitudinal and
   lateral force demand relative to available force;
3. include aggregate vertical-load sensitivity;
4. evolve heat and wear from a documented workload or dissipated-energy proxy;
5. apply the same state trajectory to lateral, braking, and drive limits;
6. solve the coupling with a fixed deterministic iteration contract;
7. publish compound parameters with explicit dimensions and plausibility
   bounds.

The Reference Model may later add surface/core temperature, four tires, slip
ratio, slip angle, pressure, graining, blistering, and wet behavior. Those are
not requirements for V3.

### 5. Aerodynamics

The quasi-steady force law `0.5 × rho × v² × effective_area` is appropriate for
both model families and is retained. The current state selection is not a
physical passive-aero law:

- four resource fields describe straight/corner drag and downforce endpoints;
- V2 interpolates between them from absolute track curvature using a cubic
  smoothstep;
- aero development scales drag and downforce together;
- the setup slider applies separate but historically unbalanced linear drag
  and downforce gains;
- a resource called `active` remains two static endpoint pairs with no command,
  actuator, transition, or energy load.

Continuous curvature was a valid improvement over a binary discontinuity, but
track geometry still selects a vehicle aerodynamic state. V3 should distinguish:

- **fixed aero**: one setup-resolved `CdA/ClA` pair, independent of circuit
  labels;
- **reduced operating response**, if retained: an explicitly named and
  calibrated function of vehicle state such as speed or lateral load, not an
  unexplained curvature label;
- **active aero**: a commanded state with bounds, transition rules, and later
  an energy load, enabled only for an era declaring the capability.

Ground effect and ride-height sensitivity can be added later. A Reference
Model may use aerodynamic maps over ride height, yaw, steering, and speed.

Model V3 candidate `0.3.0` implements the fixed-aero boundary: one resolved
`CdA/ClA` pair is used for every segment, independently of track curvature.
The transitional Simulator resolver still derives that pair from historical
fields; governed screening must replace this bridge with reviewed resources.

### 6. Combustion engine and thermal state

The torque-curve representation and linear interpolation are useful
reduced-order foundations. Power is calculated as:

```text
power = torque × rpm × pi / 30
```

The sample magnitudes imply torque in kN·m and power in kW, but the resource
schema does not make that unit contractual. V3 must do so.

Engine temperature uses one lumped thermal state:

```text
heat = alpha_heat × delivered_power
cooling = (p_cool0 + k_cool × speed) × (temperature - ambient)
temperature_next = temperature + (heat - cooling) / heat_capacity × dt
```

Above `t_soft`, available power decreases linearly to a floor of 20%. This
structure is suitable for the Game Model after units, parameters, activation
evidence, and thermal limits are reviewed. The current duplicated engine
thermal resources and inert cooling axis are not calibration evidence.

The Reference Model may later distinguish combustion, coolant, oil, motor,
inverter, and battery temperatures. V3 needs only states that create a useful,
explainable decision.

Model V3 candidate `0.3.0` keeps the lumped equation but evaluates heat from
loaded engine power and publishes generated heat, removed heat, peak
temperature, and derated duration. Cooling therefore has an inspectable
activation domain and no direct pace multiplier.

### 7. Transmission and driveline

Every spatial sample currently evaluates all gears and immediately chooses the
gear providing maximum engine power. `n_upshift` and `n_downshift` are parsed
but unused. All gearbox setup ratios are multiplied by one common factor.

The result has no shift duration, torque interruption, hysteresis, driveline
efficiency, launch model, differential, or sequential constraint. A vehicle can
therefore change directly between non-adjacent maximum-power gears.

V3 should replace this with one deterministic sequential gear state:

- explicit ratios and final drive;
- adjacent up/down shifts from a versioned controller;
- shift duration or torque interruption;
- bounded driveline efficiency;
- an explicit launch approximation;
- a setup control whose physical meaning is final-drive or ratio-set selection,
  not an anonymous multiplier.

Individual ratio design may remain outside the first Game Model UI. The
Reference Model can later include clutch, differential, and richer losses.

Model V3 candidate `0.3.0` implements adjacent deterministic shifts, resolved
RPM thresholds, shift duration and power interruption, plus driveline
efficiency. The first-ratio/idle clamp remains an explicit reduced-order launch
approximation.

### 8. Fuel, power flow, and modular hybrid energy

Current fuel mass is constant during one lap, then reduced by:

```text
fuel_burn = fuel_burn_kg_per_s × adjusted_lap_time
```

Burn is independent of engine power, throttle, efficiency, and fuel energy.
Fuel affects performance only through lap-level mass. A missing catalog field
silently becomes `0.02 kg/s`; the resource named `v6t_hybrid` uses that fallback.

This equation must be replaced for V3. The first combustion Game Model may use
a simple power-dependent specific-consumption law. It does not need a complete
combustion simulation, but consumed fuel energy must be connected to delivered
work and modeled losses.

From the first hybrid-capable era, the Game Model must expose a simplified but
real modular energy flow:

```text
battery_energy_next
  = battery_energy
  + recovered_energy × charge_efficiency
  - deployed_energy / discharge_efficiency
```

with, at minimum:

- bounded state of charge;
- bounded charge, discharge, and regenerative power;
- propulsion and auxiliary loads;
- deterministic reserve and derating behavior;
- no energy creation within a declared residual tolerance;
- automatic deployment before strategic player or agent control;
- telemetry for store, source, recovery, delivery, loss, reserve, and residual.

Module absence is explicit. Early cars do not carry a battery object filled
with zeroes. A resolved powertrain should use a tagged capability such as
combustion-only or hybrid rather than a bag of optional fields.

The Reference Model may later add operating-point efficiency maps, dynamic
thermal limits, battery ageing, and richer fuel flow. The energy state and
conservation vocabulary should be shared with the future Pod/Drone bridge only
after Racing proves it; no generic crate is extracted yet.

### 9. Driver representation

Aggressiveness currently changes base tire wear, deterministic lap-time noise,
and a peak pace bonus. The pace and noise milliseconds are applied after the
physical lap is solved by scaling its time axis. Speed, power, temperature,
tire state, and the spatial solution are not recomputed. Fuel burn then uses
the adjusted time while tire and thermal evolution came from the unadjusted
solve.

This is deterministic but not a coherent physical transform. V3 should replace
post-solve pace scaling with bounded control parameters that act before or
during the solve, for example:

- fraction of the available braking, cornering, and traction envelope used;
- deterministic braking/turn-in control error;
- tire-workload or consistency trade-off;
- reaction or shift behavior where the model supports it.

The Simulator may still add racecraft or incident behavior later. An external
agent must receive the same public observations and bounded actions as a human
or authored controller; it receives no private pace multiplier.

Model V3 candidate `0.3.0` implements bounded braking, cornering and traction
utilization plus deterministic control error inside the force solve. Its V3
time axis is no longer scaled by a driver bonus or noise after solving.

### 10. Pit stops and race orchestration

The Solver resets wear, chooses a new tire resource, resets tire temperature,
and advances logical time. The Simulator adds one circuit/policy pit loss to
the scored lap. This separation avoids double-scoring the loss, but every named
production circuit currently inherits the same 22-second fallback.

Competitors are simulated independently in clean air and ranked by total time.
There is no grid, traffic, overtaking, dirty air, slipstream, safety car,
weather, failure, or incident interaction. Every successful row is `Finished`.

This independent-race approximation may remain in V3 because it keeps the game
approachable and the workload distributable. It must be named in the scenario
or model profile. Circuit-specific pit loss, tire-change state, and strategy
must be explicit resources. Wheel-to-wheel interaction is a later Simulator
capability, not a reason to weaken the physical Solver now.

### 11. Catalog validation and telemetry

The catalog boundary correctly validates immutable bytes, indexes, digests,
references, and model compatibility. Physical parsers, however, mostly require
fields without enforcing domain bounds or cross-field relationships. Examples
include positive mass and wheel radius, ordered RPM axes, non-negative force
areas, tire-factor bounds, thermal ordering, fuel rate, and gear-count
consistency.

V3 resources require strict, unit-bearing schema validation before a snapshot
can enter the Solver. Silent defaults for physical data must be removed unless
the value is a documented universal constant.

Current telemetry exposes speed, power, RPM, gear, temperatures, tire state,
and g-forces, but not fuel mass/flow, force utilization, normal load, energy
flows, modeled loss, or conservation residual. Those diagnostic channels are
required before related capabilities can be calibrated or explained to a
player.

### 12. Current test evidence

The existing suite strongly protects deterministic repetition, catalog byte
identity, model/catalog compatibility, V1/V2 golden output, continuous-aero
bounds, and the historical tuning transform. Those are necessary replay and
delivery properties.

They are not yet a physical validation suite. The reviewed kernel tests do not
directly establish:

- one consistent combined tire-force budget in acceleration and braking;
- tire heat, wear, and load sensitivity over a race distance;
- a power-dependent fuel-consumption invariant;
- shift-state sequencing or shift losses;
- engine cooling activation and energy conservation;
- physical consistency between driver pace, elapsed time, and state evolution;
- plausibility bounds across every catalog component.

Golden tests preserve published behavior even when that behavior is an
approximation. V3 therefore needs new invariant and metamorphic tests rather
than replacing golden compatibility with subjective realism assertions.

## Era-gated module plan

Capabilities accumulate; they are not retroactively applied to every vehicle.

| Era | Game Model modules introduced or emphasized | Deliberately absent |
| ---: | --- | --- |
| 1 | point-mass longitudinal solve, combustion torque curve, sequential gearbox, aggregate tire state | battery, active aero, flight loads |
| 2 | reviewed fixed aero and repeatable component resolution | strategic energy |
| 3 | circuit-dependent fixed-aero/ground-effect approximation with distinct drag and downforce development | battery, commanded active aero |
| 4 | aggregate tire load sensitivity, mass, braking, and thermal material consequences | hidden material bonus |
| 5 | power-based fuel flow, battery store, regeneration, losses, automatic deployment, reserve telemetry | player energy micromanagement |
| 6 | bounded strategic deployment/recovery, active-system loads, commanded active aero, public agent boundary | private AI pace modifiers |
| 7 | fixed-path Pod/Drone mission, lift/propulsion/auxiliary loads, battery thermal and terminal reserve | free trajectory and flight control |

An era may introduce presentation and economy before all future physical detail,
but it cannot claim an active capability that its resolved model does not run.

## Proposed V3 delivery sequence

### Foundation A — resolved physics boundary

1. define strict units and validation for V3 physical resources;
2. move gameplay point resolution out of the Solver;
3. define model/module identities and reject incompatible resources;
4. preserve V1/V2 golden replay paths unchanged.

The first offline implementation of this boundary is documented in
[Racing Game Model V3 — Foundation Candidate](RACING_MODEL_V3_FOUNDATION.md).
It uses a candidate namespace until the complete production V3 capability set
and catalog compatibility have been reviewed.

### Foundation B — coherent mechanical core

1. use explicit segment distances and re-derive vertical-load terms;
2. apply one combined tire-force utilization rule to braking and acceleration;
3. introduce aggregate load sensitivity;
4. replace tire heat/wear workload and make transient coupling consistent;
5. introduce sequential transmission and a named brake limit;
6. replace post-solve driver time scaling with physical envelope utilization.

This foundation comes before a broad coefficient campaign. Otherwise a sweep
would calibrate incompatible state paths.

### Foundation C — reviewed calibration resources

Issue #245 then externalizes only coefficients whose governing equations,
units, bounds, and activation domains have passed this audit. Small deterministic
equivalence fixtures protect unchanged compatibility plumbing; they do not
require V3 to reproduce V2 physics.

### Foundation D — decision-surface evidence

Issue #244 first screens locally, then freezes a governed Databricks campaign.
It measures marginal effects, interactions, saturation, activation, circuit
specificity, progression, and held-out robustness for the exact Game Model and
resource identities.

### Foundation E — energy

Issue #246 adds combustion energy accounting and the modular hybrid flow in
diagnostics-first stages. Automatic control precedes strategic control. Issue
#247 later reuses the proven concepts for a fixed-path Pod/Drone energy model.

## Verification strategy

Different tests protect different claims:

### Historical compatibility

- V1/V2 golden native and WASM outputs remain exact;
- Authority and Verifier continue to authorize their published identities;
- catalog releases are never edited in place.

### V3 equation and invariant tests

- dimensional and finite-value validation;
- non-negative mass, elapsed time, wear, and stored energy;
- bounded force utilization, state of charge, power, and temperature behavior;
- combined-force consistency in both acceleration and braking;
- energy source/store/load/loss residual within tolerance;
- deterministic fixed-iteration and failure behavior;
- pit transitions change only declared state;
- telemetry is derived from the same solved state.

### Metamorphic tests

Subject to an explicitly controlled scenario:

- increasing drag alone cannot increase terminal straight-line speed;
- reducing available grip cannot improve the physical speed envelope;
- adding mass cannot improve level-ground acceleration through an unmodeled
  bonus;
- a zero-capability early-era vehicle cannot deploy or recover energy;
- deployment cannot exceed available storage or declared power;
- increasing an isolated pit loss increases scored race time by that amount.

Metamorphic properties require carefully isolated inputs; they are not global
claims about full race strategy.

### Calibration and validation evidence

- local sensitivity screening before shared compute;
- governed Databricks campaigns only from frozen manifests and binaries;
- representative and held-out circuits;
- multiple seeds only where stochastic behavior is material;
- explicit comparison between Game and Reference outputs once the Reference
  Model exists;
- operational telemetry is calibration evidence, never an online dependency.

## Dependency decisions

The reviewed order is:

1. parameter inventory (#243);
2. this approximation and fidelity audit (#251);
3. enabled-era vehicle reconciliation (#248);
4. reviewed physical-resolution and coefficient resources (#245);
5. decision-surface screening and Databricks campaign (#244);
6. hybrid energy foundation (#246);
7. Pod/Drone bridge (#247);
8. Opponent Policy V3 from reviewed competitive frontiers.

Vehicle recovery may be investigated in parallel, but no Era 3/4 candidate is
published against equations whose physical meaning has not been reviewed.

## Non-goals

This audit does not:

- implement V3;
- select final tire, aero, thermal, or energy coefficients;
- claim that a reduced-order model will reproduce real Formula 1 telemetry;
- require four-wheel, CFD, combustion, or flight-control fidelity;
- enable Eras 6 or 7;
- make Databricks part of online gameplay;
- create a generic energy crate before a second working domain proves the
  abstraction.

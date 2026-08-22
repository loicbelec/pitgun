# Racing Game Model V3 — Foundation Candidate

Status: offline candidate `pitgun.racing-v3-candidate@0.6.0`. It is not
authorized by a production catalog and cannot be selected by the game,
Authority, or Verifier.

## Purpose

This first executable V3 slice establishes two foundations that Model V2 could
not change without breaking historical replay:

1. the Solver receives a physically resolved vehicle rather than game
   development points and setup sliders;
2. the spatial solve integrates every explicit track segment instead of
   assuming that the first sample spacing applies to the complete circuit.

The candidate now also contains the first coherent aggregate contact-patch
model and physically interpretable mechanical controls. It intentionally does
not attempt to calibrate opponent pace. Its role is to make tire,
transmission, braking, driver, aerodynamic, and thermal changes physically
attributable and measurable. Energy accounting remains a later slice.

## Ownership boundary

```text
game progression + setup + immutable response resource
                         |
                         v
              Racing Simulator resolver
                         |
                         v
            resolved physical vehicle (SI)
                         |
                         v
                Racing Solver V3 candidate
```

`ResolvedSimulationRequestV3` has no tuning field. Development points, prices,
unlocks, doctrines, and era economy therefore cannot enter the V3 Solver
boundary. The current offline Simulator adapter resolves the historical
response explicitly before calling it. A future V3 parameter resource will
replace that transitional response before production publication.

The Solver validates finite physical inputs and the main admissible domains for
mass, wheel radius, grip, aerodynamic areas, torque curves, gearing, thermal
coefficients, tires, initial state, and simulation configuration. Existing
serialized field names remain available to historical V1/V2 paths; validation
messages document their SI meaning.

## Segment-aware integration

For V1/V2, `SimConfig.ds` retains its historical meaning: one spacing value is
used throughout the lap. This compatibility path is unchanged.

For the V3 candidate, segment `i` uses:

```text
delta_s_i = s_(i+1) - s_i
```

That local distance is used for braking reachability, forward acceleration,
time steps, thermal and tire-state duration, longitudinal acceleration, and
final time integration. Vertical-path acceleration uses the slope derivative
with respect to the actual distance coordinate:

```text
a_vertical_i = velocity_i^2 * d(slope)/ds_i
```

The candidate ignores the compatibility `config.ds` field. Strictly increasing
finite distance samples are required.

## Aggregate tire/contact patch

V3 still represents all four tires as one reduced-order contact patch. It does
not pretend to resolve individual wheels, slip angle, slip ratio, pressure, or
carcass layers. Within that declared scope it now uses one physical budget:

```text
F_available = mu(state) * Fz * (Fz / Fz_reference)^(-load_sensitivity)
F_longitudinal_available = sqrt(max(F_available^2 - F_lateral^2, 0))
utilization = clamp(hypot(F_longitudinal, F_lateral) / F_available, 0, 1)
```

The same temperature/wear trajectory is used by the corner envelope, backward
braking pass, and forward traction pass. Their nonlinear coupling is resolved
with exactly four deterministic iterations per lap. This is a reproducibility
contract, not a convergence tolerance that could stop differently across
runtimes.

Temperature and wear no longer depend on squared accelerations with unclear
units. They evolve from a named bounded contact-workload power proxy:

```text
contact_workload_power = F_available * velocity * utilization^2
heat_power = heat_generation_fraction * contact_workload_power
cooling_power = (cooling_static + cooling_speed * velocity)
                * (tire_temperature - ambient_temperature)
wear_delta = baseline_wear_rate * dt
             + contact_workload_power * dt / full_wear_energy
```

The candidate coefficients declare SI dimensions and fail closed outside their
reviewed bounds. They are explicit resolved Solver inputs but are not promoted
to catalog resources yet; Databricks screening must establish useful ranges
before that publication step. Compound grip, temperature window and wear-grip
response continue to come from the selected tire resource.

The Solver exposes per-sample combined utilization, normal load and available
force plus summary heat/workload diagnostics. These are calibration evidence,
not hidden pace multipliers.

## Aerodynamic efficiency

The `0.4.0` candidate replaces the transitional linear setup response with one
fixed aerodynamic state resolved before the Solver. The setup selects `ClA`;
the corresponding `CdA` follows a reduced-order quadratic polar:

```text
ClA = ClA_reference * setup_multiplier * development_downforce_gain
CdA = (CdA_reference * base_drag_multiplier
       + induced_drag_factor * added_ClA^2)
      * development_drag_reduction
```

The six resolution coefficients are explicit in experiment profile V2.
Aerodynamic development improves the resulting `ClA/CdA` ratio rather than
scaling drag and downforce together. The Solver still receives only the final
fixed `CdA` and `ClA`; it has no knowledge of sliders or development points.

The five-by-five local setup grid produces three distinct representative
optima in candidate `0.4.0`: Monza `0.50/0.25`, Monaco `1.00/0.25`, and Suzuka `0.75/0.50`
(downforce/gearing). This removes universal setup dominance without embedding
circuit-specific rules. The evidence is documented in
[`RACING_V3_AERO_EFFICIENCY_V1.md`](RACING_V3_AERO_EFFICIENCY_V1.md).

## Development resolution

The `0.5.0` candidate removes the transitional shortcut that made chassis
points multiply the tire-friction coefficient. Tire `mu0` now remains an
unchanged tire property. Chassis progression resolves a bounded suspension and
contact-patch force-transfer efficiency in `(0, 1]`; engine progression resolves
torque gain; cooling progression resolves heat-rejection capacity. All three
quantities are explicit experiment-profile coefficients.

Cooling still has no direct time bonus: a coefficient is considered physically
active when it changes the governed thermal or derating observables, even if a
three-lap time is unchanged. The 738-run local screen removes sole chassis
dominance and makes engine allocation circuit-dependent. The initial values
remain candidates rather than calibrated truths. Full interpretation and
evidence are in
[`RACING_V3_DEVELOPMENT_RESOLUTION_V1.md`](RACING_V3_DEVELOPMENT_RESOLUTION_V1.md).

## Transmission resolution

The `0.6.0` candidate replaces the opaque common gear-ratio multiplier with an
explicit final-drive calculation. The normalized setup selects a target top
speed; maximum engine speed, wheel radius and the catalog top-gear ratio then
determine one common multiplier. Internal gearbox spacing is preserved, while
the resolved theoretical top speed is exposed in diagnostics.

The 792-run local screen keeps the transmission active and circuit-dependent:
Monza, Monaco and Suzuka retain three distinct combined setup optima. The
initial `85..105 m/s` range is experimental rather than calibrated. Full
interpretation and evidence are in
[`RACING_V3_TRANSMISSION_RESOLUTION_V1.md`](RACING_V3_TRANSMISSION_RESOLUTION_V1.md).

## Mechanical controls

The `0.3.0` candidate removes four remaining shortcuts from the V3 path.
Historical V1/V2 retain them exclusively for replay compatibility.

### Sequential transmission

The Solver carries one gear state through space and time. A controller may
request only the adjacent ratio when the resolved upshift or downshift RPM is
crossed. During the resolved shift duration, delivered power is blended with
the declared shift power fraction. Wheel power also includes the resolved
driveline efficiency:

```text
wheel_power = engine_power
              * thermal_derating
              * driveline_efficiency
              * shift_power_fraction_over_segment
```

The candidate accepts an upshift threshold in RPM, a downshift threshold in
RPM, a shift duration in seconds, a delivered-power fraction during the shift,
and a driveline efficiency in `[0, 1]`. Shift duration is currently reviewed
only over `[0, 0.5] s`. There is no clutch, differential or launch controller
yet. The first ratio and clamped idle RPM form the declared launch
approximation.

### Braking and driver utilization

The backward pass now uses the smallest of three inspectable budgets:

```text
tire_longitudinal_budget = sqrt(max(F_available^2 - F_lateral^2, 0))
driver_budget = tire_longitudinal_budget * braking_utilization
braking_force = min(driver_budget, maximum_brake_force)
```

The system limit is expressed in newtons. The former compiled `6 g` ceiling is
not used by V3. Cornering and traction likewise use explicit utilization
fractions. A deterministic sample-level control error can only reduce a
driver's resolved utilization; it never adds or subtracts milliseconds after
the physical solve. Utilization is currently reviewed over `[0.5, 1]` and
control error over `[0, 0.25]`.

The Simulator's offline adapter translates the historical aggressiveness
field to these physical controls. That mapping is transitional and belongs to
the Simulator, not the Solver. A future catalog resource will replace it.

### Fixed aerodynamics

V3 accepts one fixed drag area `CdA` and one fixed downforce area `ClA`, both in
square metres. Track curvature no longer selects or blends an aerodynamic
state. Drag and downforce therefore remain independently resolvable before the
Solver boundary. The current Simulator adapter derives the fixed pair from the
mean of the transitional straight/corner vehicle fields; reviewed V3 resources
will store the pair directly.

### Observable cooling

Engine cooling still uses the reviewed reduced-order lumped thermal equation:

```text
heat_power = heat_fraction * loaded_engine_power
cooling_power = (static_cooling + speed_cooling * velocity)
                * (engine_temperature - ambient_temperature)
```

There is no hidden cooling pace multiplier. Cooling can affect pace only when
temperature crosses the declared soft limit and activates thermal derating.
V3 now reports generated heat, removed heat, maximum engine temperature and
time spent derated, making both the inactive and active domains observable.

Per-sample evidence additionally includes brake-force budget, the three
resolved driver-utilization channels, engine derating factor and delivered
shift-power fraction.
Summary evidence reports brake-limit activation, sequential shifts, shift
interruption time and driveline loss.

## Identity and compatibility

- historical `pitgun.racing@1.0.0` and `pitgun.racing@2.0.0` remain unchanged;
- mechanical-controls profile V1 retains `pitgun.racing-v3-candidate@0.3.0`;
- aerodynamic-efficiency profile V2 uses `pitgun.racing-v3-candidate@0.4.0`;
- development-resolution profile V3 uses `pitgun.racing-v3-candidate@0.5.0`;
- transmission-resolution profile V4 uses `pitgun.racing-v3-candidate@0.6.0`;
- active-vehicle profile V5 uses `pitgun.racing-v3-candidate@0.7.0`;
- power-based fuel-mass profile V6 uses `pitgun.racing-v3-candidate@0.8.0`;
- compound-degradation profile V7 uses `pitgun.racing-v3-candidate@0.9.0`;
- engine-thermal experiment profile V8 uses `pitgun.racing-v3-candidate@0.10.0`;
- component-composed profile V9 uses `pitgun.racing-v3-candidate@0.11.0`;
- `pitgun.racing@3.0.0` is reserved for the reviewed production Game Model;
- no existing Racing Catalog declares compatibility with the candidate;
- browser, Authority, and Verifier cannot select it.

This prevents an exploratory equation set from masquerading as a published
model while retaining deterministic identity for offline evidence.

## Evidence in this slice

- identical resolved V3 inputs produce identical outputs;
- changing the historical `config.ds` value cannot change V3 output;
- non-uniform grids integrate their explicit total distance;
- malformed segment coordinates and invalid physical vehicles fail closed;
- the aggregate tire/contact equations intentionally make V3 output distinct
  from V2 even on a uniform flat fixture;
- combined utilization, tire temperature, and wear remain finite and bounded;
- less available grip cannot improve the V3 result;
- gear selection changes only to an adjacent ratio and shift duration has a
  measurable cost;
- braking never exceeds the named resolved system limit;
- driver identity has no pace effect when resolved utilization and error are
  identical;
- increasing drag alone cannot improve isolated terminal speed;
- cooling is visible only through heat removal, temperature and derating;
- chassis development leaves tire friction unchanged and exposes its resolved
  force-transfer efficiency;
- historical V1/V2 tests continue to protect compatibility.

## Next physics slices

1. explicit transmission and gear-ratio setup semantics;
2. held-out and multi-era screening followed by governed Databricks replay;
3. reviewed V3 catalog resources and production identity;
4. explicit combustion/hybrid energy accounting.

Distinct historical Era 3/4 vehicles are not restored by this foundation.
Their reviewed physical resources should be introduced only after the V3
equations that interpret them are stable.

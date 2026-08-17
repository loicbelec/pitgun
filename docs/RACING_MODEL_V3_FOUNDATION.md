# Racing Game Model V3 — Foundation Candidate

Status: offline candidate `pitgun.racing-v3-candidate@0.2.0`. It is not
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
model. It intentionally does not attempt to calibrate opponent pace. Its role
is to make tire, transmission, braking, driver, thermal, and energy changes
physically attributable and measurable.

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

## Identity and compatibility

- historical `pitgun.racing@1.0.0` and `pitgun.racing@2.0.0` remain unchanged;
- the incomplete candidate uses `pitgun.racing-v3-candidate@0.2.0`;
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
- historical V1/V2 tests continue to protect compatibility.

## Next physics slices

1. sequential transmission, named braking limit, physical driver utilization,
   fixed-aero separation, and observable cooling;
2. local screening followed by the governed multi-era Databricks campaign;
3. reviewed V3 catalog resources and production identity;
4. explicit combustion/hybrid energy accounting.

Distinct historical Era 3/4 vehicles are not restored by this foundation.
Their reviewed physical resources should be introduced only after the V3
equations that interpret them are stable.

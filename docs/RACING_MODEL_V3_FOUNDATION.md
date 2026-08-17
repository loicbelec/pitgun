# Racing Game Model V3 — Foundation Candidate

Status: offline candidate `pitgun.racing-v3-candidate@0.1.0`. It is not
authorized by a production catalog and cannot be selected by the game,
Authority, or Verifier.

## Purpose

This first executable V3 slice establishes two foundations that Model V2 could
not change without breaking historical replay:

1. the Solver receives a physically resolved vehicle rather than game
   development points and setup sliders;
2. the spatial solve integrates every explicit track segment instead of
   assuming that the first sample spacing applies to the complete circuit.

The slice intentionally does not attempt to calibrate opponent pace. Its role
is to make the next tire, transmission, braking, driver, thermal, and energy
changes physically attributable and measurable.

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

## Identity and compatibility

- historical `pitgun.racing@1.0.0` and `pitgun.racing@2.0.0` remain unchanged;
- the incomplete candidate uses `pitgun.racing-v3-candidate@0.1.0`;
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
- uniform, flat fixtures may remain numerically equal to V2 but retain a
  distinct candidate identity;
- historical V1/V2 tests continue to protect compatibility.

## Next physics slices

1. coherent aggregate tire/contact-patch state and combined force use;
2. sequential transmission, named braking limit, physical driver utilization,
   fixed-aero separation, and observable cooling;
3. local screening followed by the governed multi-era Databricks campaign;
4. reviewed V3 catalog resources and production identity;
5. explicit combustion/hybrid energy accounting.

Distinct historical Era 3/4 vehicles are not restored by this foundation.
Their reviewed physical resources should be introduced only after the V3
equations that interpret them are stable.

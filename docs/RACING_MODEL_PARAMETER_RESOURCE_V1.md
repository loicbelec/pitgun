# Racing Model Parameter Resource V1

## Status and purpose

`pitgun.racing-model-parameters/v1` is the strict immutable resource boundary
for the historical Racing Game Model V2 response. It separates reviewed
calibrable values from equations without claiming that moving a value out of
Rust improves the physics.

The first fixture is a compatibility representation of
`TuningResponseV1::default()`. Historical catalog and model identities keep
their compiled behavior. A later catalog release may resolve this resource
explicitly only after deterministic equivalence is proved.

Two purposes are closed into V1:

- `model-v2-compatibility` records the historical production values;
- `model-v2-offline-candidate` enables governed screening without changing a
  production catalog or mutable `LATEST` pointer.

The resource is compatible only with `pitgun.racing@2.0.0`. New equations or
new parameter semantics require a new model identity and, when incompatible,
a new resource schema.

## Ownership boundary

The resource groups values by what they control:

1. `development_resolution` maps game points to physical multipliers;
2. `setup_response` maps normalized setup sliders to physical multipliers;
3. `aerodynamic_state_response` carries two coefficients retained by the
   Model V2 aerodynamic equation.

Numerical safety details, iteration counts, integration floors, state guards,
prices, unlock rules, and UI concepts are deliberately absent. Equations stay
in the versioned Rust Solver workload.

## Field semantics and provenance

Every V1 numeric range is a transport admissibility range. It rejects corrupt
or absurd resources early; it is not evidence that every admitted value is
physically plausible. Candidate promotion still requires the governed
experiments described by #244.

| Resource field | Unit | V1 bounds | Model V2 source | Owner |
| --- | --- | --- | --- | --- |
| `development_resolution.points_cap_per_axis` | development points/axis | `[1, 100]` | `development_points_cap` | gameplay-to-physical resolver |
| `development_resolution.aerodynamic_area_gain_at_cap` | ratio | `[0, 4]` | `aero_development_gain` | gameplay-to-physical resolver |
| `development_resolution.chassis_grip_gain_at_cap` | ratio | `[0, 4]` | `chassis_grip_development_gain` | gameplay-to-physical resolver |
| `development_resolution.cooling_base_multiplier` | ratio | `[0.1, 4]` | `cooling_base` | gameplay-to-physical resolver |
| `development_resolution.cooling_gain_at_cap` | ratio | `[0, 4]` | `cooling_development_gain` | gameplay-to-physical resolver |
| `development_resolution.engine_torque_gain_at_cap` | ratio | `[0, 4]` | `engine_torque_development_gain` | gameplay-to-physical resolver |
| `setup_response.drag_area_base_multiplier` | ratio | `[0.1, 4]` | `drag_base` | physical setup response |
| `setup_response.drag_area_slider_gain` | ratio | `[0, 4]` | `drag_slider_gain` | physical setup response |
| `setup_response.downforce_area_base_multiplier` | ratio | `[0.1, 4]` | `downforce_base` | physical setup response |
| `setup_response.downforce_area_slider_gain` | ratio | `[0, 4]` | `downforce_slider_gain` | physical setup response |
| `setup_response.gear_ratio_base_multiplier` | ratio | `[0.1, 4]` | `gear_ratio_base` | physical setup response |
| `setup_response.gear_ratio_slider_reduction` | ratio | `[0, 4)` and below base | `gear_ratio_slider_reduction` | physical setup response |
| `aerodynamic_state_response.straight_multiplier` | ratio | `[0.1, 4]` | `straight_aero_scale` | Model V2 equation coefficient |
| `aerodynamic_state_response.corner_multiplier` | ratio | `[0.1, 4]` | `corner_aero_scale` | Model V2 equation coefficient |

The source inventory and physical review are
`RACING_PARAMETER_INVENTORY_V1.md` and
`RACING_MODEL_APPROXIMATION_AUDIT_V1.md`. The compatibility fixture lives at
`crates/pitgun-racing-contract/tests/fixtures/racing_model_parameters_v1.json`.

## Identity and immutability

The resource carries a stable semantic `identity` and exact
`compatible_model`. Its canonical content digest is declared by the immutable
Simulation Pack index. The signed run already binds the executable model and
Simulation Pack identities; the resolver must reject any missing, malformed,
tampered, or incompatible parameter resource before simulation.

A coefficient change therefore produces new resource bytes, a new resource
digest, a new Simulation Pack identity, and a new immutable catalog release.
It never mutates an existing replay lineage.

## Catalog-backed compatibility release

Racing Catalog `1.4.0` is the first release to index
`simulation/model-parameters/v2-compatibility.json`. Its exact bytes reproduce
the reviewed Model V2 fixture. Catalogs `1.3.0` and earlier retain their
compiled compatibility behavior and are never reinterpreted through this new
resource.

The simulator resolves the resource only after its index digest, path,
identity, schema, numeric bounds, model compatibility, and catalog
compatibility have passed validation. A missing, altered, malformed, or
incompatible resource fails before simulation. Rust experiments may inject an
explicit validated offline-candidate resource without changing a catalog or
the public `LATEST` selection.

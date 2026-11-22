# Thermal driver V1 — ability, commitment and accounted consequences

Native opt-in candidate, 22 November 2026. Implements part of
[#455](https://github.com/loicbelec/pitgun/issues/455) and
[#456](https://github.com/loicbelec/pitgun/issues/456).
It does not select a new catalog, browser workload or Authority admission.
Qualification and limitations: [local experiment](../../experiments/thermal_driver_v1/README.md).

## Driver characteristics

There are **three persistent physical abilities**, each in `[0,1]`, and an
independent requested driving mode. A single generic “talent” score would hide
the actual tradeoffs. These traits already existed in the V2 resource contract;
this increment connects them explicitly to the thermal/energy candidate.

| Parameter | Meaning | Physical effect in this candidate |
| --- | --- | --- |
| `limit_exploitation` | Ability to use available grip | Raises useful cornering, braking and traction demand. Does not create grip or engine power. |
| `consistency` | Repeatability of execution | Reduces seeded control-error amplitude, correction workload and modeled hesitation hazard. |
| `tire_management` | Ability to avoid wasteful corrections | Reduces the additional correction demand, dissipated work, heat and attributed wear. No direct magic grip bonus or wear discount. |
| Manage / Balanced / Attack | Requested commitment, not talent | Uses catalog commitment `.6 / .8 / 1`. Raises useful demand, but also error, corrections and hesitation hazard. Fixed for this experiment; no new player slider. |

Catalog1.8.1 archetypes, in exploitation / consistency / management order:

| Archetype | Values |
| --- | --- |
| Balanced reference | `.84 / .84 / .84` |
| Limit specialist | `.96 / .78 / .78` |
| Smooth operator | `.78 / .96 / .78` |
| Tire manager | `.78 / .78 / .96` |

The equal trait sum2.52 is an authoring convention, not evidence of equal race
strength. These names are resource identities, not an automatically calibrated
opponent lineup. Both player and rivals can use the same resolver. The intended
initial player mode remains explicitly Balanced when future admission is wired.

No separate fatigue, morale, overtaking, wet-weather, reaction-time or accident
resistance trait is implemented here. Setup points, tires, technology and pit
strategy are vehicle/controller inputs, not hidden driver abilities.

## Equations and force budget

Let `L,C,M` be the three abilities and `c` requested commitment. Reuse the
existing governed V2 mapping:

- Channel demand `u = floor + span L c`, with corner `.80/.20`, brake `.78/.22`,
  traction `.82/.18`.
- Control-error amplitude `e = .005 + .08 c² (1-C)`.
- Correction-work multiplier `q = 1 + 2 c e (1-M)`.

Seeded small control errors reduce useful channel demand. A higher commitment
therefore **does not improve execution precision**, unlike the old V9 equation
`error=.04-.03*aggressiveness`. Ordinary utilization noise remains distinct from
discrete recoverable hesitations.

The new `EnergyAccountedV1` reservation leaves useful aggregate force capacity
`A/sqrt(q)`, where `A` is the actual tire/chassis capacity. Braking and traction
use `sqrt((A/sqrt(q))² - F_lateral²)` before their channel utilization. Scaling
only the old longitudinal remainder would overbook the combined circle while
cornering. The acceptance check retains the real physical capacity and verifies
the correction-inclusive bound, without relaxing thermal coupling tolerances.

Thermal V2's reduced deformation law uses the accepted useful contact demand.
Its deformation coefficient becomes `k*q`: correction losses are paid in the
longitudinal energy solve, then passed to the existing two-node tire heat and
work/temperature wear model. Additional deformation work is attributed as
`W_correction = W_deformation * (1 - 1/q)`; corresponding heat and wear are
reported separately. No post-solve time penalty, unaccounted extra heat or direct
wear subtraction is applied.

These equations are a reduced control/deformation convention, not a calibrated
human biomechanics or detailed tire-slip model. Increased error/correction
parameters do not imply every complete Attack race must finish with more total
wear: time, temperature, speed, fuel and services also change.

## First recoverable fault

This increment adds a **temporary reduction of requested engine torque**, a
throttle hesitation. It changes the solved motion, work, fuel and thermal state;
it is neither a random lap-time addition nor an accident marker without physics.
The engine's true thermal derating remains separately recorded.

Exploratory policy, pinned before qualification:

`hazard_per_100km = .02 c² + 40 c⁴ (1-C)²`.

Distance exposure is divided into500m cells. A purpose-separated hash of
`version, seed, actor noise identity, cell index` produces a strictly interior
uniform draw. Exponential first arrival determines whether/where an onset falls
inside that cell: `p(cell)=1-exp(-hazard*500/100000)`. At most one onset per cell;
this is **not** a full Poisson event process. No draw occurs per render frame,
export sample, numerical iteration, retry, boundary poll or LLM call.

Each onset has a250m spatial footprint with engine-request fraction `.15`.
The overlap is distance-averaged over each native segment; overlapping footprints
form a union. This preserves integrated request reduction under spatial
partitioning, not exact nonlinear trajectory invariance to changing the native
mesh. The footprint is a game convention, not a measured reaction duration.
It can straddle a lap/service boundary in distance coordinates; service consumes
no distance, and this first spatial model does not infer recovery from pit time.

The first native accepted point at/after the known onset records an observation
time. This is not a root-exact onset timestamp. Facts are emitted once from
accepted trajectories, never coupling retries. An onset under braking can have
little/no torque effect; event counts alone are not a time-loss measure.

The profile and onset facts are serialized in the native solution; canonical
input/output identities include them. They are not yet converted into the
shared `RacingSessionEventV1` journal or displayed in the Pit Wall. Collision,
spin, DNF, weather, VSC, grip-dependent incident hazard and their shared-car
frontier are subsequent capabilities, not claims of this increment.

## Versioning, reproducibility and remaining admission

`thermal_driver::resolve_thermal_driver_v1` requires an exact **V2** resource;
unknown/legacy-only IDs fail instead of silently selecting `default`. The
resource and policy have separate digests. The actor's noise identity is distinct
from named ability, allowing one-trait comparisons on the same stream.

`apply_thermal_driver_v1` atomically validates/applies a resolution to an explicit
energy/thermal request. The old `Driver.aggressiveness` field becomes compatibility
metadata in this opt-in path; native behavior comes from resolved controls,
correction model and lapse profile. Dynamic mode schedules remain rejected by
the energy model. This local helper is not a trust/admission boundary: a future
Authority must resolve and bind the policy/resource itself rather than trust a
caller-supplied resolution/digest.

New optional wire fields are omitted for historical profiles. Existing admitted
fallbacks, catalog bytes and model selection remain unchanged. Reproduction from
serialized initial inputs and in-memory continuation at pre-service boundaries
are tested; this is not a new arbitrary process-checkpoint implementation or a
native/WASM parity claim.

Next: qualify the incident law against actual demand/grip, full process journal
and runtime/export replay; perform the local opponent campaign with legal fuel
and real HQ progression fixtures; freeze coefficients and reserved evaluation;
then admit a new explicit runtime/catalog version. No claim that the existing
game can no longer be won through concentrated engine/chassis allocation.

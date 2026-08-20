# Racing Model V3 development resolution V1

Status: reviewed local candidate `pitgun.racing-v3-candidate@0.5.0`. It is not
published to a catalog and cannot be selected by the game, Authority or
Verifier.

## Problem found by the 0.4.0 screen

At a constant forty-point budget, moving points toward chassis was faster on
Monza, Monaco and Suzuka. The cause was structural: the transitional Model V2
resolver multiplied the vehicle's raw tire-friction coefficient by chassis
development. One gameplay axis therefore improved cornering, braking and
traction simultaneously.

Increasing AI budgets or changing opponent profiles could only hide this
surface. It could not make the player's development decision meaningful.

## V3 separation

Profile V3 keeps the catalog tire coefficient unchanged. Chassis development
instead resolves one bounded mechanical quantity:

```text
force_transfer_efficiency = lerp(base_efficiency, cap_efficiency, chassis_points / points_cap)
available_contact_force = theoretical_tire_force * force_transfer_efficiency
```

The efficiency represents the reduced-order ability of suspension and chassis
to transfer the theoretical aggregate contact-patch force. It must satisfy
`0 < base <= cap <= 1`, is passed to the Solver as a mechanical parameter, and
is returned in mechanical diagnostics. It is deliberately not a new tire
coefficient.

Engine and cooling progression are also explicit:

```text
torque = reference_torque * (1 + torque_gain_at_cap * engine_points / points_cap)
cooling_capacity = reference_capacity
                   * (base_multiplier + gain_at_cap * cooling_points / points_cap)
```

Cooling has no direct pace bonus. It changes heat rejection, temperature and
thermal derating only. Aerodynamic development retains the reviewed `0.4.0`
efficiency law. Gear-ratio resolution remains transitional and is intentionally
reserved for a separate slice.

## Identity and replay

- profile V1 still selects mechanical candidate `0.3.0`;
- profile V2 still selects aerodynamic candidate `0.4.0`;
- profile V3 requires both aerodynamic and development resources and selects
  candidate `0.5.0`;
- missing or contradictory version/resource combinations fail closed;
- V1 and V2 profiles and reports remain unchanged.

## Local evidence

The V3 campaign contains 738 deterministic three-lap executions: three seeds,
three representative circuits, twenty-two isolated physical controls, a
constant development budget, and the five-by-five setup grid.

| Development move at fixed budget | Monza | Monaco | Suzuka |
| --- | ---: | ---: | ---: |
| More aero | -423 ms | -128 ms | -649 ms |
| More chassis | -746 ms | -1169 ms | -1364 ms |
| More engine | -461 ms | +165 ms | -32 ms |
| More cooling | +1725 ms | +1188 ms | +2004 ms |

Negative values are faster. Chassis is no longer the only profitable move:
aero is competitive everywhere and engine allocation becomes
circuit-dependent. Cooling remains a preventive thermal choice rather than a
three-lap pace upgrade. Its screened gain changes peak temperature by roughly
14–15 °C and reduces derating, so it is physically active even when elapsed
time is unchanged.

The specialization verdict moves from `STRUCTURAL_CHANGE_REQUIRED` to
`REFINE`. This is not yet a calibration claim: held-out circuits, longer runs,
active-era vehicles and governed Databricks replay are still required.

The setup grid continues to produce circuit-specific optima:

| Circuit | Reviewed optimum (downforce / gearing) |
| --- | --- |
| Monza | `0.50 / 0.25` |
| Monaco | `1.00 / 0.00` |
| Suzuka | `1.00 / 0.25` |

## Decision

Accept the physical separation and the candidate identity. Do not promote its
initial coefficient values to the game or use them to regenerate opponent
profiles yet.

Next:

1. extend the campaign to held-out circuits, longer thermal windows and active
   eras;
2. replay the accepted candidate through Delta and MLflow on Databricks;
3. publish reviewed parameter resources only after those gates pass.

The explicit transmission slice is now reviewed in
[`RACING_V3_TRANSMISSION_RESOLUTION_V1.md`](RACING_V3_TRANSMISSION_RESOLUTION_V1.md).

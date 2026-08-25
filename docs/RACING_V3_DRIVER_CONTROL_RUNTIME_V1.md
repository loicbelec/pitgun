# Racing Model V3 driver-control runtime V1

Status: offline candidate implementation under
[`loicbelec/pitgun#311`](https://github.com/loicbelec/pitgun/issues/311).

This slice introduces `pitgun.racing-v3-candidate@0.12.0`. It is deliberately
absent from hosted workload selection and does not modify the published
`0.11.0` model or Racing catalog `1.6.0`.

## Resolution boundary

The Simulator receives, through an offline-only experiment boundary:

- one V2 driver resource for every selected `driver_id`;
- one explicit `manage`, `balanced`, or `attack` mode per competitor;
- one V1 driver-control coefficient profile;
- the unchanged component and power-unit thermal resources used by `0.11.0`.

It validates those resources and resolves the contract equations to:

- cornering, braking and traction utilization limits;
- deterministic sample-level control-error amplitude;
- a correction-workload multiplier greater than or equal to one.

The Solver receives only those physical values. It does not know player-facing
mode labels and does not apply a lap-time bonus.

## Physical cost

The existing contact workload is retained as `base_workload`. Candidate
`0.12.0` adds:

```text
correction_workload
  = base_workload * (correction_workload_multiplier - 1)

total_workload = base_workload + correction_workload
```

`total_workload` enters the existing tire heat and compound-wear equations.
The correction term therefore changes pace only through tire state. It cannot
reduce the underlying workload and it remains deterministic for identical
inputs.

## Diagnostics

Offline race output now records, for each competitor:

- exact traits and selected mode;
- requested commitment;
- resolved utilization for all three force channels;
- control-error amplitude;
- correction-workload multiplier.

Tire diagnostics additionally distinguish base workload, correction workload,
correction heat and correction-attributed wear. Older models omit these
optional fields, preserving their wire representation and numerical results.

## Promotion gate

The candidate is not accepted by `racing_model_identity_for_version`, catalog
loading, WASM, Authority or Verifier. Promotion requires the local screening
and Databricks campaign defined in
[`RACING_V3_DRIVER_CONTROL_CONTRACT_V1.md`](RACING_V3_DRIVER_CONTROL_CONTRACT_V1.md),
followed by a new immutable catalog and verification package.

Catalog `1.8.0` and Model `0.14.0` now satisfy the publication half of this
gate without changing any deployed selection. They bind the reviewed
equal-budget V2 drivers, the physical coefficient profile, and the instruction
profile in one Simulation Pack. Authorized native/WASM execution and hosted
Verifier replay remain separate follow-up gates.

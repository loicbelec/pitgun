# Racing Model V3 fuel and mass validation

This bounded offline campaign validates candidate `0.8.0` after the held-out
`0.7.0` vehicle/tire campaign. It does not deploy a model or calibrate an
opponent policy.

The reduced-order combustion law integrates engine output work and converts it
to consumed mass with brake-specific fuel consumption, plus a small idle flow:

```text
fuel_used_kg
  = engine_output_work_kWh × specific_consumption_kg_per_kWh
  + elapsed_time_s × idle_fuel_flow_kg_per_s
```

Fuel mass remains constant inside one solved lap and updates the total vehicle
mass for the following lap. The report exposes the mass after each lap so that
this approximation is measurable rather than hidden. Empty or insufficient
fuel fails closed instead of allowing propulsion work without an energy source.

The campaign covers four held-out circuits, four active vehicles and initial
loads of 40, 70 and 100 kg: 48 deterministic executions and 576 simulated
laps. It checks accounting closure, monotonic fuel state and the pace response
to initial mass.

```bash
cargo build --release -p pitgun-racing-simulator --example v3_validation_probe
python3 experiments/racing_v3_fuel_mass/validate_local.py --jobs 4
python3 experiments/racing_v3_fuel_mass/validate_local.py --jobs 4 --check
```

One specific-consumption law is deliberately shared across eras in this first
slice. That is a documented Game Model approximation, not a calibration claim.
Engine-specific efficiency and fuel-flow limits can be introduced later under
new parameter and model identities. Hybrid storage, recovery and deployment
remain in #246.

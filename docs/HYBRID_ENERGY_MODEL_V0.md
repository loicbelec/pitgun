# Hybrid Energy Model v0

This note defines the first energy-management extension for the Pitgun lap-time
simulation model.

## Current Solver Shape

`pitgun-solver` currently models lap time from resolved, deterministic inputs:

- track geometry: distance, position, curvature, slope, heading
- vehicle parameters: chassis, aero, engine, tires
- tuning: aero, chassis, cooling, engine, downforce, gear ratio
- state: fuel mass, tire wear, tire temperature, engine temperature, exit speed,
  exit gear
- strategy/runtime inputs: lap count, pit plan, driver profile, telemetry rate

The QSS flow is split into two domains:

1. Space domain: compute the speed profile from corner limits, braking pass,
   acceleration pass, and final speed merge.
2. Time domain: integrate the speed profile into lap time and resample telemetry.

The existing powertrain is combustion-only. Engine power is derived from RPM and
torque samples, then constrained by thermal derating, traction, drag, rolling
resistance, and slope.

## Design Goals

- Preserve deterministic behavior for the same resolved inputs.
- Add battery state as real vehicle state, not a UI-only multiplier.
- Keep v0 small enough to test and tune.
- Emit telemetry that can explain why a lap changed.
- Avoid introducing optimization or control complexity before the gameplay loop
  proves it needs it.

## Non-Goals

- No full electrical machine model.
- No cell-level battery chemistry.
- No stochastic energy deployment.
- No server-side ML or learned controller in v0.
- No divergence between `pitgun-solver` and `pitgun-simulator`.

## New Concepts

### Hybrid System Parameters

The vehicle needs optional hybrid parameters:

- `battery_capacity_kwh`: usable battery energy.
- `battery_min_soc`: lower operating bound, in `[0, 1]`.
- `battery_max_soc`: upper operating bound, in `[0, 1]`.
- `max_deploy_kw`: maximum electrical deployment power.
- `max_regen_kw`: maximum regenerative braking power.
- `deploy_efficiency`: battery-to-wheel efficiency.
- `regen_efficiency`: wheel-to-battery efficiency.
- `mass_kg`: hybrid system mass penalty.

The safest integration point is `VehicleParams`, either as:

```rust
pub hybrid: Option<HybridParams>
```

or as a defaulted field if compatibility requires it:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub hybrid: Option<HybridParams>
```

### Hybrid State

`VehicleState` should carry battery state:

```rust
#[serde(default = "default_battery_soc")]
pub battery_soc: f64
```

`battery_soc` is normalized in `[0, 1]`. For non-hybrid cars it remains `0.0` or
is ignored. For hybrid cars, defaults should be deterministic and documented by
era or vehicle config.

### Deployment Mode

v0 should expose a small deterministic strategy surface:

- `balanced`: deploy on acceleration zones while preserving charge.
- `attack`: deploy more aggressively until `battery_min_soc`.
- `harvest`: reduce deployment and favor regeneration.

This can start as a request-level enum:

```rust
pub enum EnergyMode {
    Balanced,
    Attack,
    Harvest,
}
```

Default should be `Balanced`.

## Solver Behavior v0

### Forward Pass

During acceleration, combustion power is computed as today:

```text
p_engine_kw = best_power_at_speed(...) * derating_factor(...)
```

Hybrid deployment then adds bounded electrical power:

```text
p_deploy_kw = f(mode, speed, throttle demand, soc)
p_total_kw = p_engine_kw + p_deploy_kw
```

Constraints:

- `p_deploy_kw <= max_deploy_kw`
- no deployment below `battery_min_soc`
- energy used over step:
  `delta_kwh = p_deploy_kw * dt / 3600 / deploy_efficiency`
- `battery_soc` decreases by `delta_kwh / battery_capacity_kwh`

### Braking Pass / Regeneration

Regeneration should be computed where the final profile indicates braking or
negative longitudinal acceleration. v0 can apply regen during the time-domain
post-pass rather than changing braking distance:

```text
p_regen_kw = f(braking_power, max_regen_kw, soc)
```

Constraints:

- `p_regen_kw <= max_regen_kw`
- no regeneration above `battery_max_soc`
- recovered energy:
  `delta_kwh = p_regen_kw * dt / 3600 * regen_efficiency`
- `battery_soc` increases by `delta_kwh / battery_capacity_kwh`

This keeps v0 simple: deployment affects acceleration and lap time, while regen
updates state and telemetry without making braking performance depend on hybrid
hardware yet.

## Telemetry

The solver should emit these optional fields in `SimulationSolution` and
`ResampledTelemetry`:

- `battery_soc_pct`
- `hybrid_deploy_kw`
- `hybrid_regen_kw`

Gateway parameter IDs and `sim.*` dictionary entries should be added only after
the solver fields are stable.

Suggested canonical metric names:

- `sim.battery_soc_pct`
- `sim.hybrid_deploy_kw`
- `sim.hybrid_regen_kw`

## Data Pack Changes

The simulator data pack should introduce hybrid-capable vehicle records without
forcing old vehicles to define hybrid fields.

Recommended first target:

- `f1_2026`: hybrid enabled
- classic eras: hybrid absent

This creates a clear gameplay progression: early eras stay combustion-focused,
late eras add energy-management decisions.

## Determinism Rules

- Energy mode must be part of the simulation request or resolved contract.
- Defaults must be explicit and versioned.
- No wall-clock, random sampling, or client-only hidden state may affect energy.
- Telemetry resampling must interpolate battery/deploy/regen from solver output.
- Golden tests should pin final lap time, final SoC, and representative telemetry.

## Implementation Order

1. Add `HybridParams`, `EnergyMode`, and `battery_soc` with backward-compatible
   serde defaults.
2. Add no-op plumbing: non-hybrid vehicles produce identical results to current
   solver output.
3. Add deployment in the forward pass.
4. Add SoC series to `SimulationSolution`.
5. Add regeneration as a post-pass state update.
6. Add resampled telemetry fields.
7. Add data-pack support in `pitgun-simulator`.
8. Add gateway dictionary entries once signal names settle.

## Acceptance Criteria

- Existing non-hybrid tests still pass without fixture churn.
- Non-hybrid vehicles produce the same lap time as before.
- Hybrid vehicle with `Attack` mode is faster than `Harvest` on power-sensitive
  tracks, assuming enough initial SoC.
- Final SoC is bounded by `[battery_min_soc, battery_max_soc]`.
- Telemetry explains deployment and regeneration over the lap.

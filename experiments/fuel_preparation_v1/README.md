# Native automatic fuel preparation V1

Local-only increment after driver PR459; issue445. No public workload/catalog,
browser Worker, game UI, signed admission or deployment selects this capability.

## Explicit policy, bounded estimate

`profile-v1.json` is an **experimental operational limit**, not a claim about
historical/F1 regulation or physical tank size:110kg maximum initial charge,
1kg finish reserve,8% provisional uncertainty, upward0.5kg rounding, two pilot
laps for each distinct compound already in the initial plan (at most3).
The policy and12-case plan are committed before measurement. No BSFC, thermal,
driver, tire or tank coefficient is fitted to make the screen pass.

Preparation uses the actual resolved vehicle/controls/states. Each pilot starts
at the declared maximum mass, without stops, and keeps the greatest observed
fuel consumption per meter. Maximum rate across planned compounds × race
distance, declared uncertainty, service idle fuel and finish reserve determine
the initial charge. The final pilot history is not materialized: preparation
stops at its last committed lap. A full53-lap race is never a preparation step.
A one-lap session necessarily needs its one-lap pilot; at most six laps are
computed for a multi-compound full-GP estimate.

Receipts bind source/policy/pilot/compound/prepared-input identities. The source
request is not mutated. Excess demand returns `Infeasible`, without a prepared
request, bigger tank, suppressed consumption or in-race refuelling. Numerical
pilot failure remains a technical error, not an estimate of fuel demand.

This is an estimate of the **initial** plan, not an upper-bound proof over all
future tire states, maneuvers, strategic changes, temperatures or native meshes.
The uncertainty coefficient needs reserved evaluation; it is not a guaranteed
safe margin. Do not present an accepted estimate as a physically validated tank
capacity or a certified guarantee of finishing. All failures stay in receipts.

## Fuel outcome and isolation

Native Solver failures now distinguish a structured fuel shortfall from a
technical failure. Existing `advance`, `advance_service` and monolithic APIs
still return their original error strings and preserve their result semantics.

The explicit `FuelAwareSessionV1` adapter turns only a typed fuel shortfall into
`FuelNotCompleted`, alongside the **last fully committed lap/service boundary**
and the budget that failed. It does not parse an error string. The failed lap
and an incomplete service are excluded; positive fuel may remain in the saved
prefix. This is a conservative lap-budget outcome, **not the exact instant or
position of a car running out of fuel**. No zero-fuel state, elapsed driving,
accident location, finish time or partial-lap telemetry is invented. Failed
private kernel work is discarded when the future terminates.

The offline `run_fuel_aware_field_v1` advances1..10 independent competitors with
the same circuit/horizon. A fuel outcome closes that car while others continue.
Technical errors still fail closed. The healthy player's complete result digest
must equal its standalone execution. This adapter does not yet replace public
standings, emit a shared DNF event, support interactive pre-service decisions or
implement a full process checkpoint. Those require a new domain/runtime contract.

## Qualification

`fuel_preparation_probe` uses frozen thermal inputs and explicit V2 driver
resolution. It records original-load outcomes separately from estimated-load
outcomes and preserves any infeasible/technical/fuel result. The driver bank's
56 original inputs/outcomes are compared exactly, including the five frozen
thermal baselines and three full-GP unchanged-service comparisons.

```sh
cargo build --offline --locked --release -p pitgun-racing-simulator \
  --example fuel_preparation_probe --example thermal_driver_probe
python3 experiments/fuel_preparation_v1/qualify_local.py \
  target/release/examples /tmp/pitgun-fuel-qualification
```

Use a clean committed source and a new destination. Test policies are explicit
inputs; no public fuel contract is overwritten. The original missing-seed
production observation in issue445 is not retrospectively reproduced by these
new synthetic native cases.

Next gates: initial-plan prediction/reserve and all-era feasibility, fine-grained
fuel-stop/domain result semantics and shared journal, boundary look-ahead/cost,
real-career allocation fixtures and opponent fitting, independent native/WASM/
Authority admission. Keep the modest accepted early cooling effect unchanged.

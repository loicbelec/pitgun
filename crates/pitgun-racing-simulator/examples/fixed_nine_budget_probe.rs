//! Offline fixed-nine budget calibration. Frozen physics; no runtime admission.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_contract::{CompetitorStintStrategy, RacingDrivingMode};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, qualified_career,
    thermal_driver::{
        ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_resource_v1,
        resolve_thermal_driver_v1,
    },
    tire_operating::{TireOperatingPolicyV1, compile_tire_operating_v1},
};
use pitgun_racing_solver::{
    ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    lap_boundary::{BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1},
    thermal_physics::ThermalLedgerV2,
    tire_temperature_grip_factor, tire_wear_grip_factor,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    policy_id: String,
    circuit: String,
    seed: u64,
    fixture_id: String,
    actor: String,
    kind: String,
    effective_budget: u16,
    effective_allocation: [u16; 4],
    downforce: f64,
    gear: f64,
    mode: RacingDrivingMode,
    strategy: CompetitorStintStrategy,
}

fn execute(policy: &TireOperatingPolicyV1, case: &Case, audit_only: bool) -> Result<Value, String> {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../experiments/opponent_physics_v2/results/career-fixtures-v1.json"
    ))
    .map_err(|e| e.to_string())?;
    let fixture = fixtures["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"].as_str() == Some(case.fixture_id.as_str()))
        .ok_or("missing retained career fixture")?;
    let era = fixture["era"].as_i64().ok_or("missing era")? as i32;
    if ![1, 2].contains(&era) {
        return Err("unsupported era".into());
    }
    let fixture_offsets: [u64; 4] = ["aero", "chassis", "engine", "cooling"]
        .map(|axis| fixture["offsets"][axis].as_u64().unwrap());
    let discretionary = fixture["discretionaryPoints"].as_u64().unwrap();
    let fixture_budget = discretionary + fixture_offsets.iter().sum::<u64>();
    let (offsets, components, min_downforce, max_downforce) = match case.kind.as_str() {
        "career" => {
            if case.actor != "balanced_reference"
                || u64::from(case.effective_budget) != fixture_budget
            {
                return Err(
                    "career case requires actual fixture budget and reference player".into(),
                );
            }
            (
                fixture_offsets,
                fixture["technology"]["components"].clone(),
                fixture["sliders"]["downforce"][0].as_f64().unwrap(),
                fixture["sliders"]["downforce"][1].as_f64().unwrap(),
            )
        }
        "field" => {
            let (lo, hi) = if era == 1 { (8, 12) } else { (12, 24) };
            if !(lo..=hi).contains(&case.effective_budget) {
                return Err("unqualified effective field budget".into());
            }
            if era == 1 {
                ([0, 0, 0, 0], json!(null), 0.5, 0.5)
            } else {
                (
                    [2, 1, 4, 1],
                    json!({"schema_version":"pitgun.racing-vehicle-components/v1","engine_id":"v8_1970","aero_id":"basic"}),
                    0.0,
                    1.0,
                )
            }
        }
        _ => return Err("unknown budget kind".into()),
    };
    let a = case.effective_allocation;
    let effective_budget = u64::from(case.effective_budget);
    if a.iter().map(|v| u64::from(*v)).sum::<u64>() != effective_budget
        || a.iter()
            .zip(offsets)
            .any(|(p, f)| u64::from(*p) < f || *p > 20)
    {
        return Err(
            "effective allocation must respect exact available cap, floors and axis limits".into(),
        );
    }
    if !case.downforce.is_finite() || !(min_downforce..=max_downforce).contains(&case.downforce) {
        return Err("downforce setting unavailable for installed components".into());
    }
    let catalog = RacingCatalogSnapshot::embedded_qualified_career().map_err(|e| e.to_string())?;
    let base = qualified_career::profile(&catalog)?;
    let context = base
        .contexts
        .iter()
        .find(|c| c.era == era && c.circuit == case.circuit)
        .ok_or("missing fixture-era circuit context")?;
    let laps: u16 = context.cases[0]
        .strategy
        .stints
        .iter()
        .map(|s| s.laps)
        .sum();
    let mut wire = fixture["preparedRequests"][0]["request"].clone();
    // Resolve installed components and bonus floors from the retained fixture,
    // never infer an upgraded player engine merely from the era number.
    wire["input"]["competitor_vehicle_components"] = if components.is_null() {
        json!({})
    } else {
        json!({"player":components})
    };
    wire["seed"] = json!(case.seed);
    wire["input"]["seed"] = json!(case.seed);
    wire["input"]["track_id"] = json!(case.circuit);
    wire["input"]["laps"] = json!(laps);
    wire["input"]["pit_strategy"] = json!({"player_pit_laps":case.strategy.pit_laps});
    let tuning = json!({"aero_points":a[0],"chassis_points":a[1],"engine_points":a[2],"cooling_points":a[3],"downforce_slider":case.downforce,"gear_ratio_slider":case.gear});
    wire["input"]["player_tuning"] = tuning.clone();
    let player = &mut wire["input"]["competitors"][0];
    player["budget_cap"] = json!(effective_budget);
    player["tuning"] = tuning;
    player["stint_strategy"] = serde_json::to_value(&case.strategy).map_err(|e| e.to_string())?;
    // Remove ignored legacy presentation fields rather than retaining stale points.
    player.as_object_mut().unwrap().remove("points");
    let request: RunRaceRequest = serde_json::from_value(wire).map_err(|e| e.to_string())?;
    let request = qualified_career::prepare_request(request, &catalog)?;
    let mut input = qualified_career::resolve_inputs(&request, &catalog)?
        .remove("player")
        .ok_or("missing player")?;
    let compiled = compile_tire_operating_v1(&catalog, &base.thermal, policy)?;
    let ids: Vec<_> = case
        .strategy
        .stints
        .iter()
        .map(|s| s.tire_id.clone())
        .collect();
    compiled
        .performance
        .apply_initial_to_input(&mut input, &ids)?;
    let driver_policy = ThermalDriverPolicyV1::from_catalog(&catalog)?;
    let stream = input
        .driver_lapses_v1
        .as_ref()
        .unwrap()
        .noise_stream_id
        .clone();
    let driver = if case.actor == "balanced_reference" {
        resolve_thermal_driver_v1(
            &catalog,
            "balanced_reference",
            case.mode,
            &driver_policy,
            &stream,
        )?
    } else {
        let person = base.roster.get(&case.actor).ok_or("unknown fixed actor")?;
        if case.mode != person.mode {
            return Err("fixed actor mode must remain frozen".into());
        }
        resolve_thermal_driver_resource_v1(
            &person.resource,
            person.mode,
            &driver_policy,
            &format!("fixed-field:{}", case.actor),
        )?
    };
    apply_thermal_driver_v1(&mut input, &driver)?;
    let physical_digest = canonical_json_digest(&input).map_err(|e| e.to_string())?;
    let receipt = json!({"policy_id":case.policy_id,"resolved_compounds":input.thermal_physics_v2.as_ref().unwrap().compounds,"policy_digest":compiled.policy_digest,"physical_input_digest":physical_digest,"request_digest":canonical_json_digest(&request).map_err(|e|e.to_string())?,"laps":laps,"budget":effective_budget,"fixture_available_effective_budget":fixture_budget,"kind":case.kind,"actor":case.actor,"bonus_offsets":offsets,"points":a,"fixture_id":case.fixture_id,"fixture_digest":canonical_json_digest(fixture).map_err(|e|e.to_string())?,"components":components,"driver":driver,"initial_state":input.state,"engine":input.vehicle.engine,"mechanical":input.mechanical,"pit_loss_s":input.config.pit_time_penalty_s,"stops":input.pit_plan.stops.iter().map(|s|s.lap).collect::<Vec<_>>()});
    if audit_only {
        return Ok(json!({"case":case,"receipt":receipt,"resolved_only":true}));
    }
    let mut expected = input.state.clone();
    let mut speed = 0.;
    let mut carcass = input
        .thermal_physics_v2
        .as_ref()
        .unwrap()
        .initial_carcass_temperature_c;
    let mut session = ResolvedSimulationSessionV3::new_boundary_diagnostic(
        BoundaryDiagnosticRequestV1 {
            schema_version: BoundaryDiagnosticVersionV1::V1,
            forecast: BoundaryForecastV1::ReachableWearV1,
            recompute_invariant: false,
            input: input.clone(),
        },
        false,
    )?;
    let mut rows = Vec::new();
    let mut jumps = [0.0_f64; 5];
    let mut ledger = ThermalLedgerV2::default();
    let mut pit_resets = true;
    let outcome = loop {
        match session.advance() {
            Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                let s = &lap.solution;
                let start = [
                    s.v[0],
                    s.temp[0],
                    s.tire_temp[0],
                    s.tire_carcass_temp[0],
                    s.tire_wear[0],
                ];
                let previous = [
                    speed,
                    expected.engine_temp,
                    expected.tire_temp,
                    carcass,
                    expected.tire_wear,
                ];
                for i in 0..5 {
                    jumps[i] = jumps[i].max((start[i] - previous[i]).abs());
                }
                let tire = input
                    .pit_plan
                    .stops
                    .iter()
                    .rev()
                    .find(|p| p.lap < lap.lap_index)
                    .map(|p| &p.tire)
                    .unwrap_or(&input.vehicle.tire);
                let (mut duration, mut ts, mut tb, mut peak, mut derated) = (0., 0., 0., 0., 0.);
                let mut minimum_power = 1.0_f64;
                let (mut hot, mut cold, mut grip_sum, mut wear_grip_sum) = (0., 0., 0., 0.);
                let compound_id = &ids[input
                    .pit_plan
                    .stops
                    .iter()
                    .filter(|p| p.lap < lap.lap_index)
                    .count()];
                let [low, high] = policy.compounds[compound_id].peak_grip_window_c;
                for i in 0..s.t.len() - 1 {
                    let dt = s.t[i + 1] - s.t[i];
                    duration += dt;
                    hot += if s.tire_temp[i] > high { dt } else { 0. };
                    cold += if s.tire_temp[i] < low { dt } else { 0. };
                    grip_sum += tire_temperature_grip_factor(s.tire_temp[i], tire) * dt;
                    wear_grip_sum += tire_wear_grip_factor(s.tire_wear[i], tire) * dt;
                    ts += s.tire_temp[i] * dt;
                    tb += s.tire_carcass_temp[i] * dt;
                    if (low..=high).contains(&s.tire_temp[i]) {
                        peak += dt;
                    }
                    minimum_power = minimum_power.min(s.engine_derating_factor[i]);
                    if s.engine_derating_factor[i] < 0.999 {
                        derated += dt;
                    }
                }
                rows.push(json!({"lap":lap.lap_index,"tire_id":ids[input.pit_plan.stops.iter().filter(|p|p.lap<lap.lap_index).count()],"time_s":lap.lap_time_s,"elapsed_s":lap.cumulative_time_s,"surface_mean_c":ts/duration,"hot_fraction":hot/duration,"cold_fraction":cold/duration,"temperature_grip_mean":grip_sum/duration,"wear_grip_mean":wear_grip_sum/duration,"surface_start_c":s.tire_temp[0],"surface_end_c":s.tire_temp.last(),"surface_max_c":s.tire_temp.iter().copied().fold(f64::NEG_INFINITY,f64::max),"carcass_max_c":s.tire_carcass_temp.iter().copied().fold(f64::NEG_INFINITY,f64::max),"fuel_after_lap_kg":lap.state_after_lap.fuel_mass,"thermal_ledger":lap.thermal_v2.unwrap().ledger,"carcass_mean_c":tb/duration,"peak_window_fraction":peak/duration,"wear_start":s.tire_wear[0],"wear_end":s.tire_wear.last(),"wear_grip_end":tire_wear_grip_factor(*s.tire_wear.last().unwrap(),tire),"minimum_temperature_grip":s.tire_temp.iter().map(|t|tire_temperature_grip_factor(*t,tire)).fold(1.0_f64,f64::min),"minimum_power_fraction":minimum_power,"derated_time_s":derated,"engine_max_c":s.temp.iter().copied().fold(f64::NEG_INFINITY,f64::max),"service_completed":lap.pit_stop_completed,"driver_lapses":s.driver_lapses_v1.len()}));
                ledger.accumulate(&lap.thermal_v2.unwrap().ledger);
                expected = lap.state_after_lap;
                carcass = lap.thermal_v2.unwrap().carcass_temperature_c;
                speed = lap.end_speed_mps.unwrap_or(0.);
                if lap.pit_stop_completed {
                    pit_resets &= expected.tire_temp == policy.performance.mount_temperature_c
                        && carcass == policy.performance.mount_temperature_c
                        && expected.tire_wear == 0.;
                }
            }
            Ok(ResolvedSimulationStepV3::Complete(result)) => {
                let service = result
                    .longitudinal_energy_v1
                    .as_ref()
                    .unwrap()
                    .service_time_s;
                let drive: f64 = result.lap_times_s.iter().sum();
                let accounting = (result.total_time_s - drive - service).abs();
                let boundary_work = session.boundary_work().unwrap_or_default();
                break json!({"ok":true,"time_s":result.total_time_s,"drive_time_s":drive,"service_time_s":service,"elapsed_accounting_residual_s":accounting,"result_digest":canonical_json_digest(&result).map_err(|e|e.to_string())?,"final_state":result.final_state,"maximum_cap_updates":boundary_work.laps.iter().filter_map(|l|l.reachable.as_ref().map(|r|r.uncommitted_cap_updates)).max().unwrap_or(0),"conservative_recomputes":boundary_work.laps.iter().filter(|l|l.reachable.as_ref().is_some_and(|r|r.conservative_recompute)).count()});
            }
            Err(error) => {
                break json!({"ok":false,"error":error,"accepted_laps":rows.len(),"last_accepted_state":expected});
            }
        }
    };
    Ok(
        json!({"schema":"pitgun.fixed-nine-budget-case/v1","case":case,"receipt":receipt,"outcome":outcome,"laps":rows,"maximum_carry_jumps_speed_engine_surface_carcass_wear":jumps,"pit_resets_exact":pit_resets,"thermal_ledger":ledger}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 && !(args.len() == 3 && args[2] == "--resolve-only") {
        return Err("usage: fixed_nine_budget_probe POLICY CASE [--resolve-only]".into());
    }
    let policy: TireOperatingPolicyV1 = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let case: Case = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let output = match execute(&policy, &case, args.len() == 3) {
        Ok(output) => output,
        Err(error) => {
            json!({"schema":"pitgun.fixed-nine-budget-case/v1","case":case,"outcome":{"ok":false,"phase":"resolution","error":error}})
        }
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn case() -> Case {
        serde_json::from_value(json!({"id":"test","policy_id":"rolling-heat-065","circuit":"MONZA",
        "seed":73013,"fixture_id":"craft_engineering:era2-entry","actor":"franz_hermann",
        "kind":"field","effective_budget":12,"effective_allocation":[2,3,5,2],"downforce":0.5,"gear":0.5,
        "mode":"balanced","strategy":{"stints":[{"tire_id":"medium","laps":26},{"tire_id":"hard","laps":27}],"pit_laps":[26]}})).unwrap()
    }
    fn policy() -> TireOperatingPolicyV1 {
        serde_json::from_str(include_str!(
            "../../../experiments/thermal_balance_sensitivity_v1/rolling-heat-065.json"
        ))
        .unwrap()
    }
    #[test]
    fn field_budget_includes_floors_once_and_uses_frozen_driver() {
        let c = case();
        let v = execute(&policy(), &c, true).unwrap();
        assert_eq!(v["receipt"]["budget"], 12);
        assert_eq!(v["receipt"]["points"], json!([2, 3, 5, 2]));
        assert_eq!(v["receipt"]["components"]["engine_id"], "v8_1970");
        assert_eq!(
            v["receipt"]["driver"]["lapse_profile"]["noise_stream_id"],
            "fixed-field:franz_hermann"
        );
        let mut other = c.clone();
        other.fixture_id = "craft_engineering:era2-exit".into();
        let w = execute(&policy(), &other, true).unwrap();
        assert_eq!(
            v["receipt"]["physical_input_digest"],
            w["receipt"]["physical_input_digest"]
        );
    }
    #[test]
    fn rejects_fictitious_career_budgets_and_invalid_field_allocations() {
        let p = policy();
        let mut c = case();
        c.effective_budget = 25;
        assert!(execute(&p, &c, true).is_err());
        c = case();
        c.effective_allocation = [1, 4, 5, 2];
        assert!(execute(&p, &c, true).is_err());
        c = case();
        c.kind = "career".into();
        assert!(execute(&p, &c, true).is_err());
        c = case();
        c.fixture_id = "craft_engineering:era1-entry".into();
        c.effective_budget = 8;
        c.effective_allocation = [0, 3, 3, 2];
        c.downforce = 0.6;
        assert!(execute(&p, &c, true).is_err());
    }
}

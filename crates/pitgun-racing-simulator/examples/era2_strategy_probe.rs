//! Offline era-II qualification from retained real-career fixtures. No AI fitting.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_contract::{CompetitorStintStrategy, RacingDrivingMode};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, qualified_career,
    thermal_driver::{ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_v1},
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
    circuit: String,
    seed: u64,
    fixture_id: String,
    discretionary_allocation: [u16; 4],
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
    if ![
        "craft_engineering:era2-entry",
        "craft_engineering:era2-exit",
    ]
    .contains(&case.fixture_id.as_str())
    {
        return Err("qualification requires an explicit era-II entry or exit fixture".into());
    }
    let fixture = fixtures["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"].as_str() == Some(case.fixture_id.as_str()))
        .ok_or("missing retained career fixture")?;
    let discretionary = fixture["discretionaryPoints"]
        .as_u64()
        .ok_or("missing discretionary budget")?;
    if case
        .discretionary_allocation
        .iter()
        .map(|v| u64::from(*v))
        .sum::<u64>()
        != discretionary
    {
        return Err("allocation must spend exactly the fixture discretionary budget".into());
    }
    let offsets: [u64; 4] = ["aero", "chassis", "engine", "cooling"]
        .map(|axis| fixture["offsets"][axis].as_u64().unwrap());
    let a: Vec<u64> = case
        .discretionary_allocation
        .iter()
        .zip(offsets)
        .map(|(p, b)| u64::from(*p) + b)
        .collect();
    let effective_budget = discretionary + offsets.iter().sum::<u64>();
    let min_downforce = fixture["sliders"]["downforce"][0].as_f64().unwrap();
    let max_downforce = fixture["sliders"]["downforce"][1].as_f64().unwrap();
    if !case.downforce.is_finite() || !(min_downforce..=max_downforce).contains(&case.downforce) {
        return Err("downforce setting unavailable for installed career components".into());
    }
    let catalog = RacingCatalogSnapshot::embedded_qualified_career().map_err(|e| e.to_string())?;
    let base = qualified_career::profile(&catalog)?;
    let context = base
        .contexts
        .iter()
        .find(|c| c.era == 2 && c.circuit == case.circuit)
        .ok_or("missing era-II circuit context")?;
    let laps: u16 = context.cases[0]
        .strategy
        .stints
        .iter()
        .map(|s| s.laps)
        .sum();
    let mut wire = fixture["preparedRequests"][0]["request"].clone();
    // Resolve installed components and bonus floors from the retained fixture,
    // never infer an upgraded player engine merely from the era number.
    let components = fixture["technology"]["components"].clone();
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
    let driver = resolve_thermal_driver_v1(
        &catalog,
        "balanced_reference",
        case.mode,
        &driver_policy,
        &stream,
    )?;
    apply_thermal_driver_v1(&mut input, &driver)?;
    let physical_digest = canonical_json_digest(&input).map_err(|e| e.to_string())?;
    let receipt = json!({"policy_digest":compiled.policy_digest,"physical_input_digest":physical_digest,"request_digest":canonical_json_digest(&request).map_err(|e|e.to_string())?,"laps":laps,"budget":effective_budget,"discretionary_points":discretionary,"bonus_offsets":offsets,"points":a,"fixture_id":case.fixture_id,"fixture_digest":canonical_json_digest(fixture).map_err(|e|e.to_string())?,"components":components,"driver":driver,"initial_state":input.state,"engine":input.vehicle.engine,"mechanical":input.mechanical,"pit_loss_s":input.config.pit_time_penalty_s,"stops":input.pit_plan.stops.iter().map(|s|s.lap).collect::<Vec<_>>()});
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
                for i in 0..s.t.len() - 1 {
                    let dt = s.t[i + 1] - s.t[i];
                    duration += dt;
                    ts += s.tire_temp[i] * dt;
                    tb += s.tire_carcass_temp[i] * dt;
                    if tire_temperature_grip_factor(s.tire_temp[i], tire) >= 0.95 {
                        peak += dt;
                    }
                    minimum_power = minimum_power.min(s.engine_derating_factor[i]);
                    if s.engine_derating_factor[i] < 0.999 {
                        derated += dt;
                    }
                }
                rows.push(json!({"lap":lap.lap_index,"tire_id":ids[input.pit_plan.stops.iter().filter(|p|p.lap<lap.lap_index).count()],"time_s":lap.lap_time_s,"elapsed_s":lap.cumulative_time_s,"surface_mean_c":ts/duration,"carcass_mean_c":tb/duration,"peak_window_fraction":peak/duration,"wear_start":s.tire_wear[0],"wear_end":s.tire_wear.last(),"wear_grip_end":tire_wear_grip_factor(*s.tire_wear.last().unwrap(),tire),"minimum_temperature_grip":s.tire_temp.iter().map(|t|tire_temperature_grip_factor(*t,tire)).fold(1.0_f64,f64::min),"minimum_power_fraction":minimum_power,"derated_time_s":derated,"engine_max_c":s.temp.iter().copied().fold(f64::NEG_INFINITY,f64::max),"service_completed":lap.pit_stop_completed,"driver_lapses":s.driver_lapses_v1.len()}));
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
        json!({"schema":"pitgun.era2-strategy-qualification-case/v1","case":case,"receipt":receipt,"outcome":outcome,"laps":rows,"maximum_carry_jumps_speed_engine_surface_carcass_wear":jumps,"pit_resets_exact":pit_resets,"thermal_ledger":ledger}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 && !(args.len() == 3 && args[2] == "--resolve-only") {
        return Err("usage: era2_strategy_probe POLICY CASE [--resolve-only]".into());
    }
    let policy: TireOperatingPolicyV1 = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let case: Case = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let output = match execute(&policy, &case, args.len() == 3) {
        Ok(output) => output,
        Err(error) => {
            json!({"schema":"pitgun.era2-strategy-qualification-case/v1","case":case,"outcome":{"ok":false,"phase":"resolution","error":error}})
        }
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> TireOperatingPolicyV1 {
        serde_json::from_str(include_str!(
            "../../../experiments/three_compound_thermal_v1/candidate-v1.json"
        ))
        .unwrap()
    }
    fn case(stage: &str) -> Case {
        serde_json::from_value(json!({"id":"test","circuit":"MONTREAL","seed":73013,"fixture_id":format!("craft_engineering:era2-{stage}"),"discretionary_allocation":if stage=="entry" {vec![3,3,3,2]}else{vec![4,4,4,4]},"downforce":0.5,"gear":0.5,"mode":"balanced","strategy":{"stints":[{"tire_id":"soft","laps":35},{"tire_id":"medium","laps":35}],"pit_laps":[35]}})).unwrap()
    }
    #[test]
    fn actual_components_and_bonus_floors_survive_resolution() {
        let p = policy();
        let entry = execute(&p, &case("entry"), true).unwrap();
        let exit = execute(&p, &case("exit"), true).unwrap();
        assert_eq!(entry["receipt"]["budget"], 12);
        assert_eq!(entry["receipt"]["bonus_offsets"], json!([0, 1, 0, 0]));
        assert_eq!(entry["receipt"]["engine"]["n_max"], 11000.0);
        assert_eq!(exit["receipt"]["budget"], 24);
        assert_eq!(exit["receipt"]["bonus_offsets"], json!([2, 1, 4, 1]));
        assert_eq!(exit["receipt"]["components"]["engine_id"], "v8_1970");
        assert_eq!(exit["receipt"]["components"]["aero_id"], "basic");
        assert_eq!(exit["receipt"]["engine"]["n_max"], 13000.0);
        assert_eq!(
            entry["receipt"]["initial_state"]["fuel_mass"],
            exit["receipt"]["initial_state"]["fuel_mass"]
        );
    }
    #[test]
    fn unavailable_controls_and_fictitious_budgets_are_rejected() {
        let p = policy();
        let mut c = case("entry");
        c.downforce = 0.2;
        assert!(execute(&p, &c, true).unwrap_err().contains("unavailable"));
        c.downforce = 0.5;
        c.discretionary_allocation = [4, 4, 4, 4];
        assert!(
            execute(&p, &c, true)
                .unwrap_err()
                .contains("discretionary budget")
        );
        let mut wire = serde_json::to_value(case("entry")).unwrap();
        wire["recompute_invariant"] = json!(true);
        assert!(serde_json::from_value::<Case>(wire).is_err());
    }
}

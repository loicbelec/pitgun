//! Offline era-III probe. Shared thermal/energy solver; no game/runtime admission.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_contract::{CompetitorStintStrategy, RacingDrivingMode};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest,
    fuel_preparation::{FuelPreparationPolicyV1, prepare_fuel_v1},
    qualified_career,
    thermal_career::resolve_thermal_career_v1,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    initial_fuel_kg: Option<f64>,
}

#[cfg(test)]
fn execute(policy: &TireOperatingPolicyV1, case: &Case, audit_only: bool) -> Result<Value, String> {
    execute_with_fuel_estimate(policy, case, audit_only, None)
}

fn execute_with_fuel_estimate(
    policy: &TireOperatingPolicyV1,
    case: &Case,
    audit_only: bool,
    fuel_policy: Option<&FuelPreparationPolicyV1>,
) -> Result<Value, String> {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../experiments/era3_joint_preparation_v1/results/career-fixtures-v1.json"
    ))
    .map_err(|e| e.to_string())?;
    let fixture = fixtures["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"].as_str() == Some(case.fixture_id.as_str()))
        .ok_or("missing retained career fixture")?;
    let era = fixture["era"].as_i64().ok_or("missing era")? as i32;
    if era != 3 {
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
            let upper = fixtures["ownershipEnvelopes"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v["availableEffectiveCap"].as_u64())
                .max()
                .unwrap();
            let lower = fixtures["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    v["discretionaryPoints"].as_u64().unwrap()
                        + ["aero", "chassis", "engine", "cooling"]
                            .iter()
                            .map(|axis| v["offsets"][axis].as_u64().unwrap())
                            .sum::<u64>()
                })
                .min()
                .unwrap();
            if !(lower..=upper).contains(&u64::from(case.effective_budget)) {
                return Err("outside exported era-III effective field envelope".into());
            }
            let developed = fixtures["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == "craft_engineering:era3-developed")
                .unwrap();
            (
                ["aero", "chassis", "engine", "cooling"]
                    .map(|axis| developed["offsets"][axis].as_u64().unwrap()),
                fixtures["opponentTechnology"]["components"].clone(),
                0.0,
                1.0,
            )
        }
        _ => return Err("unknown budget kind".into()),
    };
    let a = case.effective_allocation;
    let effective_budget = u64::from(case.effective_budget);
    if a.iter().map(|v| u64::from(*v)).sum::<u64>() > effective_budget
        || a.iter()
            .zip(offsets)
            .any(|(p, f)| u64::from(*p) < f || *p > 20)
    {
        return Err("effective allocation must fit available cap, floors and axis limits".into());
    }
    if !case.downforce.is_finite() || !(min_downforce..=max_downforce).contains(&case.downforce) {
        return Err("downforce setting unavailable for installed components".into());
    }
    let catalog = RacingCatalogSnapshot::embedded_validated_career().map_err(|e| e.to_string())?;
    let base = qualified_career::profile(&catalog)?;
    let saved = fixture["preparedRequests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["circuitId"].as_str() == Some(case.circuit.as_str()))
        .ok_or("circuit outside exported regular/invitation contexts")?;
    let laps = saved["request"]["input"]["laps"]
        .as_u64()
        .ok_or("missing pinned GP laps")?;
    let mut wire = saved["request"].clone();
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
    let player = &mut wire["input"]["competitors"][0];
    player["budget_cap"] = json!(effective_budget);
    player["tuning"] = tuning;
    player["stint_strategy"] = serde_json::to_value(&case.strategy).map_err(|e| e.to_string())?;
    // Remove ignored legacy presentation fields rather than retaining stale points.
    player.as_object_mut().unwrap().remove("points");
    let request: RunRaceRequest = serde_json::from_value(wire).map_err(|e| e.to_string())?;
    let mut request = request;
    request.input.race.competitors.retain(|c| c.is_player);
    request.input.competitor_profiles.clear();
    let driver_policy = ThermalDriverPolicyV1::from_catalog(&catalog)?;
    let mut input = resolve_thermal_career_v1(
        &request,
        &catalog,
        &base.thermal,
        &driver_policy,
        &std::collections::BTreeMap::new(),
    )?
    .competitors
    .remove("player")
    .ok_or("missing player")?
    .input;
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
    if let Some(kg) = case.initial_fuel_kg {
        if !kg.is_finite() || !(1.0..=200.0).contains(&kg) {
            return Err("invalid explicit diagnostic initial fuel".into());
        }
        input.state.fuel_mass = kg;
    }
    let physical_digest = canonical_json_digest(&input).map_err(|e| e.to_string())?;
    if let Some(policy) = fuel_policy {
        let (receipt, prepared) = prepare_fuel_v1(&input, policy)?;
        return Ok(
            json!({"schema":"pitgun.era3-initial-fuel-estimate/v1","case":case,"original_physical_input_digest":physical_digest,"preparation":receipt,"prepared_input_available":prepared.is_some(),"scope":"Bounded initial-plan estimate only; no GP, capacity identification or model/runtime admission."}),
        );
    }
    let receipt = json!({"policy_id":case.policy_id,"resolved_compounds":input.thermal_physics_v2.as_ref().unwrap().compounds,"policy_digest":compiled.policy_digest,"physical_input_digest":physical_digest,"request_digest":canonical_json_digest(&request).map_err(|e|e.to_string())?,"laps":laps,"budget":effective_budget,"spent_budget":a.iter().map(|v|u64::from(*v)).sum::<u64>(),"unused_budget":effective_budget-a.iter().map(|v|u64::from(*v)).sum::<u64>(),"fixture_available_effective_budget":fixture_budget,"kind":case.kind,"actor":case.actor,"bonus_offsets":offsets,"points":a,"fixture_id":case.fixture_id,"fixture_digest":canonical_json_digest(fixture).map_err(|e|e.to_string())?,"components":components,"driver":driver,"initial_state":input.state,"engine":input.vehicle.engine,"mechanical":input.mechanical,"pit_loss_s":input.config.pit_time_penalty_s,"stops":input.pit_plan.stops.iter().map(|s|s.lap).collect::<Vec<_>>()});
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
        json!({"schema":"pitgun.era3-joint-native-case/v1","case":case,"receipt":receipt,"outcome":outcome,"laps":rows,"maximum_carry_jumps_speed_engine_surface_carcass_wear":jumps,"pit_resets_exact":pit_resets,"thermal_ledger":ledger}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2
        && !(args.len() == 3 && args[2] == "--resolve-only")
        && !(args.len() == 4 && args[2] == "--fuel-estimate")
    {
        return Err(
            "usage: era3_budget_probe POLICY CASE [--resolve-only | --fuel-estimate FUEL_POLICY]"
                .into(),
        );
    }
    let policy: TireOperatingPolicyV1 = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let case: Case = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let fuel_policy = if args.len() == 4 {
        Some(serde_json::from_slice::<FuelPreparationPolicyV1>(
            &std::fs::read(&args[3])?,
        )?)
    } else {
        None
    };
    let output = match execute_with_fuel_estimate(
        &policy,
        &case,
        args.len() == 3,
        fuel_policy.as_ref(),
    ) {
        Ok(output) => output,
        Err(error) => {
            json!({"schema":"pitgun.era3-joint-native-case/v1","case":case,"outcome":{"ok":false,"phase":"resolution","error":error}})
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
            "../../../experiments/thermal_balance_sensitivity_v1/rolling-heat-065.json"
        ))
        .unwrap()
    }
    fn case() -> Case {
        serde_json::from_value(json!({"id":"era3-test","policy_id":"rolling-heat-065","circuit":"BARCELONA","seed":83017,
 "fixture_id":"craft_engineering:era3-developed","actor":"franz_hermann","kind":"field","effective_budget":34,"effective_allocation":[7,8,12,7],"downforce":0.5,"gear":0.5,"mode":"balanced",
 "strategy":{"stints":[{"tire_id":"medium","laps":33},{"tire_id":"hard","laps":33}],"pit_laps":[33]}})).unwrap()
    }
    #[test]
    fn field_keeps_turbo_traits_and_cap() {
        let r = execute(&policy(), &case(), true).unwrap();
        assert_eq!(r["receipt"]["components"]["engine_id"], "v6t");
        assert_eq!(r["receipt"]["budget"], 34);
        assert_eq!(
            r["receipt"]["driver"]["lapse_profile"]["noise_stream_id"],
            "fixed-field:franz_hermann"
        );
    }
    #[test]
    fn entry_player_keeps_actual_v8() {
        let mut c = case();
        c.kind = "career".into();
        c.actor = "balanced_reference".into();
        c.fixture_id = "craft_engineering:era3-entry".into();
        c.effective_budget = 24;
        c.effective_allocation = [4, 6, 8, 6];
        let r = execute(&policy(), &c, true).unwrap();
        assert_eq!(r["receipt"]["components"]["engine_id"], "v8_1970");
    }
    #[test]
    fn unused_points_above_axis_saturation_are_explicit() {
        let mut c = case();
        c.effective_budget = 82;
        c.effective_allocation = [20; 4];
        let r = execute(&policy(), &c, true).unwrap();
        assert_eq!(r["receipt"]["spent_budget"], 80);
        assert_eq!(r["receipt"]["unused_budget"], 2);
    }
    #[test]
    fn rejects_unknown_contexts_caps_and_floor_violations() {
        let mut c = case();
        c.circuit = "YASMARINA".into();
        assert!(execute(&policy(), &c, true).is_err());
        c = case();
        c.effective_budget = 83;
        assert!(execute(&policy(), &c, true).is_err());
        c = case();
        c.effective_allocation = [0, 8, 19, 7];
        assert!(execute(&policy(), &c, true).is_err());
    }
    #[test]
    fn every_exported_doctrine_and_circuit_resolves_actual_components() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../experiments/era3_joint_preparation_v1/results/career-fixtures-v1.json"
        ))
        .unwrap();
        let mut count = 0;
        for fixture in fixtures["fixtures"].as_array().unwrap() {
            for saved in fixture["preparedRequests"].as_array().unwrap() {
                let mut c = case();
                c.kind = "career".into();
                c.actor = "balanced_reference".into();
                c.fixture_id = fixture["id"].as_str().unwrap().into();
                c.circuit = saved["circuitId"].as_str().unwrap().into();
                c.effective_budget = (fixture["discretionaryPoints"].as_u64().unwrap()
                    + ["aero", "chassis", "engine", "cooling"]
                        .iter()
                        .map(|a| fixture["offsets"][a].as_u64().unwrap())
                        .sum::<u64>()) as u16;
                let tuning = &saved["request"]["input"]["competitors"][0]["tuning"];
                c.effective_allocation = [
                    "aero_points",
                    "chassis_points",
                    "engine_points",
                    "cooling_points",
                ]
                .map(|a| tuning[a].as_f64().unwrap() as u16);
                let laps = saved["request"]["input"]["laps"].as_u64().unwrap() as u16;
                let split = laps / 2;
                c.strategy=serde_json::from_value(json!({"stints":[{"tire_id":"medium","laps":split},{"tire_id":"hard","laps":laps-split}],"pit_laps":[split]})).unwrap();
                let r = execute(&policy(), &c, true).unwrap();
                assert_eq!(
                    r["receipt"]["components"],
                    fixture["technology"]["components"]
                );
                assert_eq!(r["receipt"]["laps"], laps);
                count += 1;
            }
        }
        assert_eq!(count, 99);
    }
    #[test]
    fn fuel_estimation_binds_unchanged_physical_input_and_existing_policy() {
        let c = case();
        let original = execute(&policy(), &c, true).unwrap();
        let fuel_policy: FuelPreparationPolicyV1 = serde_json::from_str(include_str!(
            "../../../experiments/fuel_preparation_v1/profile-v1.json"
        ))
        .unwrap();
        let estimated =
            execute_with_fuel_estimate(&policy(), &c, false, Some(&fuel_policy)).unwrap();
        assert_eq!(
            estimated["original_physical_input_digest"],
            original["receipt"]["physical_input_digest"]
        );
        assert_eq!(
            estimated["preparation"]["source_input_digest"],
            original["receipt"]["physical_input_digest"]
        );
        assert_eq!(original["receipt"]["initial_state"]["fuel_mass"], 100.0);
        assert_eq!(
            estimated["preparation"]["pilots"].as_array().unwrap().len(),
            2
        );
    }
    #[test]
    fn explicit_fuel_counterfactual_changes_only_starting_mass() {
        let mut c = case();
        let original = execute(&policy(), &c, true).unwrap();
        c.initial_fuel_kg = Some(110.0);
        let changed = execute(&policy(), &c, true).unwrap();
        assert_eq!(changed["receipt"]["initial_state"]["fuel_mass"], 110.0);
        assert_eq!(changed["receipt"]["points"], original["receipt"]["points"]);
        assert_eq!(
            changed["receipt"]["components"],
            original["receipt"]["components"]
        );
        assert_eq!(changed["receipt"]["driver"], original["receipt"]["driver"]);
        c.initial_fuel_kg = Some(-1.0);
        assert!(execute(&policy(), &c, true).is_err());
    }
}

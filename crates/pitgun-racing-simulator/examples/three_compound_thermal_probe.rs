//! Bounded serial diagnostic; only the player's physical input is solved.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, qualified_career,
    tire_operating::{TireOperatingPolicyV1, compile_tire_operating_v1},
};
use pitgun_racing_solver::{
    ResolvedSimulationRequestV3, ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    SimulationResult,
    lap_boundary::{BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1},
    tire_temperature_grip_factor,
};
use serde_json::{Value, json};
fn solve(input: ResolvedSimulationRequestV3) -> Result<SimulationResult, String> {
    let mut previous = (
        input.state.tire_temp,
        input
            .thermal_physics_v2
            .as_ref()
            .unwrap()
            .initial_carcass_temperature_c,
    );
    let mut session = ResolvedSimulationSessionV3::new_boundary_diagnostic(
        BoundaryDiagnosticRequestV1 {
            schema_version: BoundaryDiagnosticVersionV1::V1,
            forecast: BoundaryForecastV1::ReachableWearV1,
            recompute_invariant: false,
            input,
        },
        false,
    )?;
    loop {
        match session.advance()? {
            ResolvedSimulationStepV3::Complete(r) => return Ok(*r),
            ResolvedSimulationStepV3::Lap(lap) => {
                if lap.solution.tire_temp[0] != previous.0
                    || lap.solution.tire_carcass_temp[0] != previous.1
                {
                    return Err("tire state changed at lap boundary".into());
                }
                previous = (
                    lap.state_after_lap.tire_temp,
                    lap.thermal_v2.unwrap().carcass_temperature_c,
                );
                if lap.pit_stop_completed && previous != (70., 70.) {
                    return Err("replacement tire did not use the declared preparation".into());
                }
            }
        }
    }
}
fn summarize(r: &SimulationResult, input: &ResolvedSimulationRequestV3) -> Value {
    let s = &r.solution;
    let mut laps = Vec::new();
    for lap in 1..=r.lap_times_s.len() {
        let tire = input
            .pit_plan
            .stops
            .iter()
            .filter(|stop| usize::from(stop.lap) < lap)
            .next_back()
            .map(|s| &s.tire)
            .unwrap_or(&input.vehicle.tire);
        let (mut dt, mut ts, mut tb, mut grip, mut heat, mut low, mut high, mut falls, mut rises) =
            (0., 0., 0., 0., 0., f64::INFINITY, f64::NEG_INFINITY, 0, 0);
        for i in 0..s.t.len() - 1 {
            if usize::from(s.lap_index[i]) != lap {
                continue;
            }
            let d = s.t[i + 1] - s.t[i];
            dt += d;
            ts += s.tire_temp[i] * d;
            tb += s.tire_carcass_temp[i] * d;
            grip += tire_temperature_grip_factor(s.tire_temp[i], tire) * d;
            if tire_temperature_grip_factor(s.tire_temp[i], tire) >= 0.95 {
                heat += d;
            }
            low = low.min(s.tire_temp[i]);
            high = high.max(s.tire_temp[i]);
            if s.tire_temp[i + 1] > s.tire_temp[i] + 1e-8 {
                rises += 1;
            }
            if s.tire_temp[i + 1] < s.tire_temp[i] - 1e-8 {
                falls += 1;
            }
        }
        laps.push(json!({"lap":lap,"time_s":r.lap_times_s[lap-1],"surface_mean_c":ts/dt,"carcass_mean_c":tb/dt,"surface_min_c":low,"surface_max_c":high,"temperature_grip_mean":grip/dt,"time_fraction_in_peak_window":heat/dt,"rising_segments":rises,"falling_segments":falls}));
    }
    json!({"laps":laps,"total_time_s":r.total_time_s,"final_wear":s.tire_wear.last(),"thermal_ledger":r.thermal_v2.as_ref().map(|t|t.ledger),"result_digest":canonical_json_digest(r).unwrap()})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = RacingCatalogSnapshot::embedded_qualified_career()?;
    let base = qualified_career::profile(&catalog)?;
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: three_compound_thermal_probe POLICY.json [--extended]")?;
    let extended = std::env::args().any(|a| a == "--extended");
    let policy: TireOperatingPolicyV1 = serde_json::from_slice(&std::fs::read(path)?)?;
    let compiled = compile_tire_operating_v1(&catalog, &base.thermal, &policy)?;
    let refs: Value = serde_json::from_str(include_str!(
        "../../../experiments/qualified_career_v1/native-reference-v1.json"
    ))?;
    let mut cases = Vec::new();
    for vector in refs["vectors"].as_array().unwrap() {
        let era = vector["request"]["era"]
            .as_i64()
            .unwrap_or(vector["request"]["input"]["era"].as_i64().unwrap());
        for id in ["soft", "medium", "hard"] {
            for variant in [
                "baseline",
                "window_only",
                "retained_heat",
                "air_exchange",
                "candidate",
            ] {
                let mut wire = vector["request"].clone();
                wire["input"]["laps"] = json!(5);
                wire["input"]["competitors"][0]["stint_strategy"] =
                    json!({"stints":[{"tire_id":id,"laps":5}],"pit_laps":[]});
                let req: RunRaceRequest = serde_json::from_value(wire)?;
                let req = qualified_career::prepare_request(req, &catalog)?;
                let mut input = qualified_career::resolve_inputs(&req, &catalog)?
                    .remove("player")
                    .unwrap();
                if variant != "baseline" {
                    let mut variant_policy = policy.clone();
                    if variant != "candidate" {
                        for (id, c) in &mut variant_policy.compounds {
                            let old = base.thermal.compounds[id].heat;
                            c.heat.surface_carcass_conductance_w_per_k =
                                old.surface_carcass_conductance_w_per_k;
                            if variant != "air_exchange" {
                                c.heat.surface_air_speed_conductance_w_per_k_mps =
                                    old.surface_air_speed_conductance_w_per_k_mps;
                            }
                            if variant == "window_only" {
                                c.heat.rolling_heat_fraction = old.rolling_heat_fraction;
                                c.heat.deformation_heat_fraction = old.deformation_heat_fraction;
                                c.heat.deformation_surface_fraction =
                                    old.deformation_surface_fraction;
                            }
                        }
                    }
                    let variant_compiled =
                        compile_tire_operating_v1(&catalog, &base.thermal, &variant_policy)?;
                    variant_compiled
                        .performance
                        .apply_initial_to_input(&mut input, &[id.to_string()])?;
                }
                let digest = canonical_json_digest(&input)?;
                let start = std::time::Instant::now();
                let result = solve(input.clone())?;
                let solve_wall_s = start.elapsed().as_secs_f64();
                let mut repeat = None;
                if extended && variant == "candidate" {
                    repeat = Some(
                        canonical_json_digest(&solve(input.clone())?)?
                            == canonical_json_digest(&result)?,
                    );
                }
                cases.push(json!({"era":era,"circuit":req.input.race.track_id,"compound":id,"variant":variant,"input_digest":digest,"repeat_exact":repeat,"solve_wall_s":solve_wall_s,"wall_with_repeat_s":start.elapsed().as_secs_f64(),"summary":summarize(&result,&input)}));
                eprintln!("completed era{era}/{id}/{variant}");
            }
        }
        if extended {
            for scenario in ["cold35", "hot140", "replacement", "full_gp"] {
                let mut wire = vector["request"].clone();
                let laps = if scenario == "full_gp" {
                    wire["input"]["laps"].as_u64().unwrap()
                } else {
                    5
                };
                wire["input"]["laps"] = json!(laps);
                wire["input"]["competitors"][0]["stint_strategy"] = if scenario == "replacement"
                    || scenario == "full_gp"
                {
                    let at = if scenario == "replacement" {
                        2
                    } else {
                        laps / 2
                    };
                    json!({"stints":[{"tire_id":"soft","laps":at},{"tire_id":"medium","laps":laps-at}],"pit_laps":[at]})
                } else {
                    json!({"stints":[{"tire_id":"medium","laps":laps}],"pit_laps":[]})
                };
                let req =
                    qualified_career::prepare_request(serde_json::from_value(wire)?, &catalog)?;
                let mut input = qualified_career::resolve_inputs(&req, &catalog)?
                    .remove("player")
                    .unwrap();
                let ids: Vec<_> = req.input.race.competitors[0]
                    .stint_strategy
                    .as_ref()
                    .unwrap()
                    .stints
                    .iter()
                    .map(|s| s.tire_id.clone())
                    .collect();
                compiled
                    .performance
                    .apply_initial_to_input(&mut input, &ids)?;
                if scenario == "cold35" || scenario == "hot140" {
                    let t = if scenario == "cold35" { 35. } else { 140. };
                    input.state.tire_temp = t;
                    input
                        .thermal_physics_v2
                        .as_mut()
                        .unwrap()
                        .initial_carcass_temperature_c = t;
                }
                let digest = canonical_json_digest(&input)?;
                let result = solve(input.clone())?;
                cases.push(json!({"era":era,"circuit":req.input.race.track_id,"scenario":scenario,"variant":"candidate","input_digest":digest,"summary":summarize(&result,&input)}));
                eprintln!("completed era{era}/{scenario}");
            }
        }
    }
    if extended {
        // Reserved context: never used to choose the two heat hypotheses.
        for id in ["soft", "medium", "hard"] {
            let mut wire = refs["vectors"][1]["request"].clone();
            wire["input"]["track_id"] = json!("SILVERSTONE");
            wire["input"]["laps"] = json!(5);
            wire["input"]["competitors"][0]["stint_strategy"] =
                json!({"stints":[{"tire_id":id,"laps":5}],"pit_laps":[]});
            let req = qualified_career::prepare_request(serde_json::from_value(wire)?, &catalog)?;
            let mut input = qualified_career::resolve_inputs(&req, &catalog)?
                .remove("player")
                .unwrap();
            compiled
                .performance
                .apply_initial_to_input(&mut input, &[id.into()])?;
            let result = solve(input.clone())?;
            cases.push(json!({"era":2,"circuit":"SILVERSTONE","scenario":"reserved","compound":id,"variant":"candidate","input_digest":canonical_json_digest(&input)?,"summary":summarize(&result,&input)}));
            eprintln!("completed reserved/Silverstone/{id}");
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"pitgun.three-compound-thermal-probe/v1","baseline_metadata":serde_json::from_str::<Value>(&qualified_career::qualified_career_metadata_json())?,"policy_digest":compiled.policy_digest,"scope":"Player only; two retained circuit/era references, three compounds, five laps, five window/source/air/coupling variants. Extended: six exact repeats, two cold/hot/replacement/full-GP cases each and three reserved Silverstone-II compounds. Exact surface/carcass carry and70C replacement assertions. No AI fit, runtime activation or leaderboard admission.","cases":cases})
        )?
    );
    Ok(())
}

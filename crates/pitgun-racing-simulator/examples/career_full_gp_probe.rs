//! Full-GP native diagnostic: isolated allocations, controls and service strategies.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::CompetitorStintStrategy;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest,
    thermal_career::{ThermalCareerPolicyV1, resolve_thermal_career_v1},
    thermal_driver::ThermalDriverPolicyV1,
};
use pitgun_racing_solver::{
    ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    lap_boundary::{BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, env, fs, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    fixture_id: String,
    circuit: String,
    allocation: Option<[u16; 4]>, // discretionary aero, chassis, engine, cooling
    gear: Option<f64>,
    downforce: Option<f64>,
    forecast: BoundaryForecastV1,
    #[serde(default = "player")]
    actor: String,
    strategy: Option<CompetitorStintStrategy>,
    seed: Option<u64>,
    environment: Option<Environment>,
    /// Fixed +1 point, outside the actual career budget: causal diagnostic only.
    diagnostic_increment: Option<Axis>,
}
#[derive(Clone, Copy, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Axis {
    Aero,
    Chassis,
    Engine,
    Cooling,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Environment {
    air_c: f64,
    road_c: f64,
}
fn player() -> String {
    "player".into()
}
fn digest_file(path: impl AsRef<Path>) -> Result<Digest, std::io::Error> {
    Ok(Digest::from_bytes(&fs::read(path)?))
}
fn extrema(xs: &[f64]) -> [f64; 2] {
    xs.iter()
        .fold([f64::INFINITY, f64::NEG_INFINITY], |[a, b], &v| {
            [a.min(v), b.max(v)]
        })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: career_full_gp_probe ROOT CASE".into());
    }
    let root = Path::new(&args[0]);
    let source = root.join("experiments/opponent_physics_v2/results/career-fixtures-v1.json");
    let fixtures: Value = serde_json::from_slice(&fs::read(&source)?)?;
    let snapshot = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
    let thermal: ThermalCareerPolicyV1 = serde_json::from_slice(&fs::read(
        root.join("experiments/career_physics_v1/profile-v1.json"),
    )?)?;
    let drivers = ThermalDriverPolicyV1::from_catalog(&snapshot)?;
    let case: Case = serde_json::from_slice(&fs::read(&args[1])?)?;
    if case.actor != "player" {
        return Err("this screen controls only the player".into());
    }
    if matches!(case.forecast, BoundaryForecastV1::EndStateHeldDiagnostic) {
        return Err("unsafe diagnostic forecast is excluded from this screen".into());
    }
    let fixture = fixtures["fixtures"]
        .as_array()
        .ok_or("fixtures")?
        .iter()
        .find(|f| f["id"] == case.fixture_id)
        .ok_or("unknown fixture")?;
    let prepared = fixture["preparedRequests"]
        .as_array()
        .ok_or("requests")?
        .iter()
        .find(|p| p["circuitId"] == case.circuit)
        .ok_or("circuit not in fixture")?;
    let mut request: RunRaceRequest = serde_json::from_value(prepared["request"].clone())?;
    let p = request
        .input
        .race
        .competitors
        .iter_mut()
        .find(|c| c.is_player)
        .ok_or("player")?;
    if let Some(a) = case.allocation {
        if a.iter().map(|x| u64::from(*x)).sum::<u64>()
            != fixture["discretionaryPoints"].as_u64().ok_or("budget")?
        {
            return Err("allocation must spend exactly the discretionary budget".into());
        }
        let axes = ["aero", "chassis", "engine", "cooling"];
        let mut points = [0.0; 4];
        for i in 0..4 {
            points[i] = f64::from(a[i]) + fixture["offsets"][axes[i]].as_f64().ok_or("offset")?;
            if points[i] > 20.0 {
                return Err("effective points exceed per-axis cap".into());
            }
        }
        p.tuning.aero_points = points[0];
        p.tuning.chassis_points = points[1];
        p.tuning.engine_points = points[2];
        p.tuning.cooling_points = points[3];
    }
    if let Some(axis) = case.diagnostic_increment {
        let target = match axis {
            Axis::Aero => &mut p.tuning.aero_points,
            Axis::Chassis => &mut p.tuning.chassis_points,
            Axis::Engine => &mut p.tuning.engine_points,
            Axis::Cooling => &mut p.tuning.cooling_points,
        };
        if *target >= 20.0 {
            return Err("diagnostic increment exceeds effective per-axis cap".into());
        }
        *target += 1.0;
        p.budget_cap += 1.0;
    }
    if let Some(strategy) = &case.strategy {
        p.stint_strategy = Some(strategy.clone());
        request
            .input
            .pit_strategy
            .as_mut()
            .ok_or("fixture missing pit strategy")?
            .player_pit_laps = strategy.pit_laps.clone();
    }
    for (proposed, key, target) in [
        (case.gear, "gearRatio", &mut p.tuning.gear_ratio_slider),
        (case.downforce, "downforce", &mut p.tuning.downforce_slider),
    ] {
        if let Some(value) = proposed {
            let bounds = &fixture["sliders"][key];
            if !value.is_finite()
                || value < bounds[0].as_f64().ok_or("slider min")?
                || value > bounds[1].as_f64().ok_or("slider max")?
            {
                return Err("slider outside actual career unlock bounds".into());
            }
            *target = value;
        }
    }
    if let Some(seed) = case.seed {
        request.seed = seed;
    }
    let field =
        resolve_thermal_career_v1(&request, &snapshot, &thermal, &drivers, &BTreeMap::new())?;
    let car = field.competitors.get(&case.actor).ok_or("unknown actor")?;
    let mut input = car.input.clone();
    if let Some(environment) = &case.environment {
        if !environment.air_c.is_finite()
            || !(0.0..=60.0).contains(&environment.air_c)
            || !environment.road_c.is_finite()
            || !(0.0..=80.0).contains(&environment.road_c)
        {
            return Err("environment outside declared native screen bounds".into());
        }
        input.vehicle.engine.t_amb = environment.air_c;
        input.config.tire_temp_amb = environment.air_c;
        input
            .thermal_physics_v2
            .as_mut()
            .unwrap()
            .road_temperature_c = environment.road_c;
    }
    let diagnostic = BoundaryDiagnosticRequestV1 {
        schema_version: BoundaryDiagnosticVersionV1::V1,
        forecast: case.forecast,
        recompute_invariant: false,
        input: input.clone(),
    };
    let diagnostic_digest = canonical_json_digest(&diagnostic)?;
    let mut session = ResolvedSimulationSessionV3::new_boundary_diagnostic(diagnostic, false)?;
    let mut laps = Vec::new();
    let mut state = input.state.clone();
    let mut speed = 0.0;
    let mut carcass = input
        .thermal_physics_v2
        .as_ref()
        .unwrap()
        .initial_carcass_temperature_c;
    let mut jumps = [0.0_f64; 5]; // speed, engine, surface, carcass, wear
    let outcome = loop {
        match session.advance() {
            Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                let s = &lap.solution;
                let starts = [
                    s.v[0],
                    s.temp[0],
                    s.tire_temp[0],
                    s.tire_carcass_temp[0],
                    s.tire_wear[0],
                ];
                let expected = [
                    speed,
                    state.engine_temp,
                    state.tire_temp,
                    carcass,
                    state.tire_wear,
                ];
                // Fuel has no per-point trace, so no independent fuel seam metric is reported.
                for i in 0..5 {
                    jumps[i] = jumps[i].max((starts[i] - expected[i]).abs());
                }
                let thermal = lap.thermal_v2.as_ref().unwrap();
                let derated =
                    s.t.windows(2)
                        .zip(&s.engine_derating_factor)
                        .filter(|(_, f)| **f < 0.999)
                        .map(|(t, _)| t[1] - t[0])
                        .sum::<f64>();
                laps.push(json!({"lap":lap.lap_index,"time_s":lap.lap_time_s,"start_speed_mps":s.v[0],"end_driving_speed_mps":s.v.last(),"engine_min_max_c":extrema(&s.temp),"surface_min_max_c":extrema(&s.tire_temp),"minimum_power_fraction":extrema(&s.engine_derating_factor)[0],"derated_time_s":derated,"pit":lap.pit_stop_completed,"state_after":lap.state_after_lap,"thermal":thermal}));
                state = lap.state_after_lap;
                speed = lap.end_speed_mps.unwrap_or(0.0);
                carcass = thermal.carcass_temperature_c;
            }
            Ok(ResolvedSimulationStepV3::Complete(result)) => {
                break json!({"ok":true,"result_digest":canonical_json_digest(&result)?,"time_s":result.total_time_s,"final_state":result.final_state,"thermal":result.thermal_v2,"energy":result.longitudinal_energy_v1});
            }
            Err(error) => break json!({"ok":false,"error":error,"accepted_laps":laps.len()}),
        }
    };
    let physical_parameters = json!({"vehicle":input.vehicle,"mechanical":input.mechanical,"tire_contact":input.tire_contact,"fuel_mass":input.fuel_mass,"thermal_physics_v2":input.thermal_physics_v2,"state":input.state,"pit_plan":input.pit_plan,"config":input.config});
    println!(
        "{}",
        json!({"schema":"pitgun.career-full-gp-probe/v1","id":case.id,"fixture":case.fixture_id,"circuit":case.circuit,"actor":case.actor,"diagnostic_increment":case.diagnostic_increment,"environment_override":case.environment,"allocation":case.allocation,"offsets":fixture["offsets"],"physical_parameters":physical_parameters,"forecast":case.forecast,"fixture_file_digest":digest_file(source)?,"raw_request_digest":canonical_json_digest(&prepared["request"])?,"typed_request_digest":field.source_request_digest,"thermal_policy_digest":field.thermal_policy_digest,"driver_policy_digest":field.driver_policy_digest,"catalog":snapshot.release_identity(),"engine":car.engine_id,"installed_components":car.installed_components,"driver":car.driver,"baseline_input_digest":car.baseline_input_digest,"physical_input_digest":canonical_json_digest(&input)?,"diagnostic_input_digest":diagnostic_digest,"initial_fuel_kg":input.state.fuel_mass,"initial_tire_surface_c":input.state.tire_temp,"lap_count":input.lap_count,"laps":laps,"maximum_carry_jumps":{"speed_mps":jumps[0],"engine_c":jumps[1],"surface_c":jumps[2],"carcass_c":jumps[3],"wear":jumps[4]},"outcome":outcome,"work":session.boundary_work(),"binary_digest":digest_file(env::current_exe()?)?})
    );
    Ok(())
}

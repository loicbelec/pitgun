//! Native-only paired cost/continuity diagnostic. No telemetry export or provider.
#[path = "support/frozen_thermal_input.rs"]
mod frozen;
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::RacingDrivingMode;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot,
    thermal_driver::{ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_v1},
    tire_preparation::{TirePreparationPolicyV1, prepare_tires_v1},
};
use pitgun_racing_solver::{
    ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    lap_boundary::{BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1},
};
use serde::Deserialize;
use serde_json::json;
use std::{env, fs, path::Path, time::Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    frozen_case: String,
    forecast: BoundaryForecastV1,
    recompute_invariant: bool,
    #[serde(default)]
    prepare_tires: bool,
    initial_temperature_c: Option<f64>,
    initial_wear: Option<f64>,
    seed: Option<u64>,
    mode: Option<RacingDrivingMode>,
    #[serde(default)]
    service_boundaries: bool,
    #[serde(default)]
    tail_trace: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: lap_boundary_probe ROOT CASE".into());
    }
    let root = Path::new(&args[0]);
    let case: Case = serde_json::from_slice(&fs::read(&args[1])?)?;
    let catalog = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
    let (mut input, _) = frozen::frozen_input(root, &case.frozen_case, &catalog)?;
    let preparation = if case.prepare_tires {
        let policy: TirePreparationPolicyV1 = serde_json::from_slice(&fs::read(
            root.join("experiments/tyre_operating_policy_v1/profile-v1.json"),
        )?)?;
        let (prepared, receipt) = prepare_tires_v1(&input, &policy)?;
        input = prepared;
        Some(receipt)
    } else {
        None
    };
    if let Some(t) = case.initial_temperature_c {
        if !t.is_finite() || !(0.0..=200.0).contains(&t) {
            return Err("invalid initial temperature".into());
        }
        input.state.tire_temp = t;
        input
            .thermal_physics_v2
            .as_mut()
            .unwrap()
            .initial_carcass_temperature_c = t;
    }
    if let Some(w) = case.initial_wear {
        if !w.is_finite() || !(0.0..=1.0).contains(&w) {
            return Err("invalid initial wear".into());
        }
        input.state.tire_wear = w;
    }
    if let Some(seed) = case.seed {
        input.config.sim_seed = seed;
    }
    let policy: ThermalDriverPolicyV1 = serde_json::from_slice(&fs::read(
        root.join("experiments/thermal_driver_v1/profile-v1.json"),
    )?)?;
    let driver = resolve_thermal_driver_v1(
        &catalog,
        "balanced_reference",
        case.mode.unwrap_or(RacingDrivingMode::Balanced),
        &policy,
        "thermal-driver:player",
    )?;
    apply_thermal_driver_v1(&mut input, &driver)?;
    let request = BoundaryDiagnosticRequestV1 {
        schema_version: BoundaryDiagnosticVersionV1::V1,
        forecast: case.forecast,
        recompute_invariant: case.recompute_invariant,
        input: input.clone(),
    };
    let diagnostic_input_digest = canonical_json_digest(&request)?;
    let mut session =
        ResolvedSimulationSessionV3::new_boundary_diagnostic(request, case.service_boundaries)?;
    let mut expected_speed = 0.0;
    let mut expected_state = input.state.clone();
    let mut expected_carcass = input
        .thermal_physics_v2
        .as_ref()
        .unwrap()
        .initial_carcass_temperature_c;
    let mut laps = Vec::new();
    let mut tails = Vec::new();
    let mut service_solution = None;
    let mut solver_ms = 0.0;
    let mut max_speed_jump: f64 = 0.0;
    let mut engine_jump: f64 = 0.0;
    let mut surface_jump: f64 = 0.0;
    let mut carcass_jump: f64 = 0.0;
    let mut wear_jump: f64 = 0.0;
    let length = *input.track.s.last().unwrap();
    let outcome = loop {
        let began = Instant::now();
        let step = if case.service_boundaries {
            use pitgun_racing_solver::ResolvedServiceStepV3;
            match session.advance_service() {
                Ok(ResolvedServiceStepV3::BeforeService(lap)) => {
                    service_solution = Some(lap.solution);
                    session.resolve_service_boundary(None)?;
                    solver_ms += began.elapsed().as_secs_f64() * 1000.0;
                    continue;
                }
                Ok(ResolvedServiceStepV3::Lap(mut lap)) => {
                    lap.solution = service_solution.take().ok_or("missing pre-service trace")?;
                    Ok(ResolvedSimulationStepV3::Lap(lap))
                }
                Ok(ResolvedServiceStepV3::Complete(r)) => Ok(ResolvedSimulationStepV3::Complete(r)),
                Err(e) => Err(e),
            }
        } else {
            session.advance()
        };
        solver_ms += began.elapsed().as_secs_f64() * 1000.0;
        match step {
            Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                let s = &lap.solution;
                max_speed_jump = max_speed_jump.max((s.v[0] - expected_speed).abs());
                engine_jump = engine_jump.max((s.temp[0] - expected_state.engine_temp).abs());
                surface_jump = surface_jump.max((s.tire_temp[0] - expected_state.tire_temp).abs());
                wear_jump = wear_jump.max((s.tire_wear[0] - expected_state.tire_wear).abs());
                carcass_jump = carcass_jump.max((s.tire_carcass_temp[0] - expected_carcass).abs());
                let mut tail_time = 0.0;
                let mut tail_brake = 0.0;
                let mut tail_min = f64::INFINITY;
                let mut tail_max: f64 = 0.0;
                let mut tail_curvature: f64 = 0.0;
                for i in 0..s.s.len() - 1 {
                    let overlap = (s.s[i + 1] - s.s[i].max(length - 300.0)).max(0.0);
                    if overlap > 0.0 {
                        tail_time += (s.t[i + 1] - s.t[i]) * overlap / (s.s[i + 1] - s.s[i]);
                        tail_brake += s.brake_force_budget_n[i] * overlap;
                        tail_min = tail_min.min(s.v[i]).min(s.v[i + 1]);
                        tail_max = tail_max.max(s.v[i]).max(s.v[i + 1]);
                        tail_curvature = tail_curvature.max(input.track.kappa[i].abs());
                    }
                }
                if case.tail_trace
                    && (lap.lap_index <= 3
                        || lap.lap_index.saturating_add(1) >= input.lap_count
                        || lap.pit_stop_completed)
                {
                    tails.push(json!({"lap":lap.lap_index,"columns":["distance_to_line_m","speed_kph","brake_force_n","surface_c","wear"],"points":(0..s.s.len()).filter(|&i| s.s[i]>=length-500.0).map(|i|[s.s[i]-length,s.v[i]*3.6,s.brake_force_budget_n[i],s.tire_temp[i],s.tire_wear[i]]).step_by(5).collect::<Vec<_>>()}));
                }
                laps.push(json!({"lap":lap.lap_index,"time_s":lap.lap_time_s,"elapsed_s":lap.cumulative_time_s,"start_speed_mps":s.v[0],"end_driving_speed_mps":s.v.last(),"tail_300m":{"time_s":tail_time,"friction_brake_work_j":tail_brake,"speed_range_mps":[tail_min,tail_max],"maximum_abs_curvature_per_m":tail_curvature},"pit":lap.pit_stop_completed,"after_service":lap.state_after_lap,"thermal":lap.thermal_v2}));
                expected_speed = lap.end_speed_mps.unwrap_or(0.0);
                expected_state = lap.state_after_lap;
                expected_carcass = lap.thermal_v2.unwrap().carcass_temperature_c;
            }
            Ok(ResolvedSimulationStepV3::Complete(result)) => {
                break json!({"ok":true,"result_digest":canonical_json_digest(&result)?,"time_s":result.total_time_s,"final_state":result.final_state,"energy":result.longitudinal_energy_v1,"thermal":result.thermal_v2});
            }
            Err(error) => break json!({"ok":false,"error":error,"accepted_laps":laps.len()}),
        }
    };
    println!(
        "{}",
        json!({"schema":"pitgun.lap-boundary-probe/v1","data":{"id":case.id,"physical_input_digest":canonical_json_digest(&input)?,"diagnostic_input_digest":diagnostic_input_digest,"forecast":case.forecast,"recompute_invariant":case.recompute_invariant,"driver":driver,"preparation":preparation,"tail_traces":tails,"catalog":catalog.release_identity(),"laps":laps,"outcome":outcome,"maximum_speed_jump_mps":max_speed_jump,"maximum_engine_temperature_jump_c":engine_jump,"maximum_tire_surface_jump_c":surface_jump,"maximum_tire_carcass_jump_c":carcass_jump,"maximum_wear_jump_fraction":wear_jump,"work":session.boundary_work(),
        "binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"sources":{"probe":Digest::from_bytes(include_bytes!("lap_boundary_probe.rs")),"helper":Digest::from_bytes(include_bytes!("support/frozen_thermal_input.rs")),"solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),"boundary":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lap_boundary.rs")),"reachability":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/tire_reachability.rs")),"energy":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs")),"thermal":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/thermal_physics.rs"))}},
        "performance":{"solver_ms":solver_ms,"scope":"sum of advance calls (and unchanged service resolution if selected); excludes catalog/input resolution, constructor validation, metrics and final digest/JSON serialization; native diagnostic counters included"}})
    );
    Ok(())
}

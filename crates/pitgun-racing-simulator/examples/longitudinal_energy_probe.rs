//! Isolated native physics candidate. Resolves career V9 once, then compares
//! Solver models on identical physical inputs; no provider or signed execution.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, resolve_career_physics_inputs_for_audit,
};
use pitgun_racing_solver::longitudinal_energy::LongitudinalEnergyModelV1;
use pitgun_racing_solver::{ResolvedSimulationRequestV3, Track, run_resolved_simulation_v3};
use serde_json::json;
use std::{collections::BTreeMap, env, fs, time::Instant};
fn coarsen(track: &mut Track, factor: usize) {
    let mut ids = (0..track.s.len()).step_by(factor).collect::<Vec<_>>();
    if ids.last() != Some(&(track.s.len() - 1)) {
        ids.push(track.s.len() - 1);
    }
    for values in [
        &mut track.s,
        &mut track.x,
        &mut track.y,
        &mut track.z,
        &mut track.kappa,
        &mut track.slope,
        &mut track.heading,
    ] {
        *values = ids.iter().map(|&i| values[i]).collect();
    }
}
fn run(input: &ResolvedSimulationRequestV3) -> serde_json::Value {
    let began = Instant::now();
    match run_resolved_simulation_v3(input) {
        Ok(r) => {
            let checks=r.longitudinal_energy_v1.map(|e|json!({
                "vehicle_work_residual_j":e.vehicle_balance_residual_j(),
                "mass_service_boundary_residual_j":e.boundary_balance_residual_j(),
                "shaft_work_residual_j":e.engine_shaft_work_j-e.wheel_propulsion_work_j-e.driveline_loss_j-e.clutch_slip_loss_j,
                "thermal_storage_residual_j":input.vehicle.engine.c_th*(r.final_state.engine_temp-input.state.engine_temp)-(e.generated_engine_heat_j-e.rejected_engine_heat_signed_j),
                "clock_residual_s":e.driving_time_s+e.service_time_s-r.total_time_s,
                "fuel_mass_residual_kg":input.fuel_mass.map(|f|r.fuel_mass_diagnostics_v3.as_ref().unwrap().fuel_consumed_kg-e.engine_shaft_work_j/3600000.*f.brake_specific_fuel_consumption_kg_per_kwh-(e.driving_time_s+e.service_time_s)*f.idle_fuel_flow_kg_per_s)
            }));
            json!({"ok":true,"solve_ms":began.elapsed().as_secs_f64()*1000.,"result_digest":canonical_json_digest(&r).unwrap(),
            "total_time_s":r.total_time_s,"lap_times_s":r.lap_times_s,"final_state":r.final_state,"fuel":r.fuel_mass_diagnostics_v3,
            "mechanical":r.mechanical_diagnostics_v3,"tires":r.tire_degradation_diagnostics_v3,"energy":r.longitudinal_energy_v1,"checks":checks,
            "nodes":r.solution.t.len(),"minimum_speed_mps":r.solution.v.iter().copied().fold(f64::INFINITY,f64::min),
            "maximum_speed_mps":r.solution.v.iter().copied().fold(0.,f64::max)})
        }
        Err(e) => json!({"ok":false,"error":e,"solve_ms":began.elapsed().as_secs_f64()*1000.}),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 3 || args.len() > 7 {
        return Err("Usage: longitudinal_energy_probe REQUEST CATALOG legacy|energy [coarsen1..8] [coolingScale0.25..2] [engineAmbient0..60] [smooth-driver]".into());
    }
    if !["legacy", "energy"].contains(&args[2].as_str()) {
        return Err("unknown physical model".into());
    }
    let request: RunRaceRequest = serde_json::from_slice(&fs::read(&args[0])?)?;
    if request.input.race.laps > 100 || request.input.race.competitors.len() > 10 {
        return Err("bounded native pilot: at most100 laps and10cars".into());
    }
    let catalog = RacingCatalogSnapshot::from_release_dir(&args[1])?;
    let mut inputs = resolve_career_physics_inputs_for_audit(request.clone(), &catalog)?;
    let factor = args
        .get(3)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    let cooling = args
        .get(4)
        .map(|s| s.parse::<f64>())
        .transpose()?
        .unwrap_or(1.);
    let ambient = args.get(5).map(|s| s.parse::<f64>()).transpose()?;
    if !(1..=8).contains(&factor)
        || !cooling.is_finite()
        || !(0.25..=2.0).contains(&cooling)
        || ambient.is_some_and(|a| !a.is_finite() || !(0.0..=60.0).contains(&a))
    {
        return Err("scenario out of pilot bounds".into());
    }
    let smooth_driver = args.get(6).is_some_and(|s| s == "smooth-driver");
    if args.get(6).is_some() && !smooth_driver {
        return Err("unknown driver experiment".into());
    }
    let mut results = BTreeMap::new();
    for (id, input) in &mut inputs {
        if factor > 1 {
            coarsen(&mut input.track, factor);
        }
        input.vehicle.engine.p_cool0 *= cooling;
        input.vehicle.engine.k_cool *= cooling;
        if let Some(a) = ambient {
            input.vehicle.engine.t_amb = a;
        }
        if smooth_driver {
            input.driver_control.control_error = 0.0;
        }
        input.longitudinal_energy = (args[2] == "energy").then_some(LongitudinalEnergyModelV1::V1);
        let digest = canonical_json_digest(input)?;
        results.insert(id,json!({"input_digest":digest,"resolved":{"mass_empty_kg":input.vehicle.chassis.mass_empty,"initial_fuel_kg":input.state.fuel_mass,
            "initial_temperature_c":input.state.engine_temp,"engine":input.vehicle.engine,"mechanical":input.mechanical,"fuel":input.fuel_mass,"driver_control":input.driver_control},"output":run(input)}));
    }
    let base_model = pitgun_racing_simulator::racing_model_v3_component_candidate_identity();
    let candidate_model = json!({"id":"pitgun.racing-longitudinal-energy-candidate","version":"1.0.0",
        "digest":canonical_json_digest(&json!({"tag":"pitgun.racing-longitudinal-energy/v1",
            "kernel_source":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),
            "energy_source":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs"))}))?});
    println!(
        "{}",
        json!({"schema":"pitgun.longitudinal-energy-pilot/v1","base_model":base_model,
            "selected_model":if args[2]=="energy" {candidate_model}else{serde_json::to_value(base_model)?},"mode":args[2],"request_digest":canonical_json_digest(&request)?,
        "catalog":catalog.release_identity(),"smooth_driver":smooth_driver,"base_resolution":"career V9/component0.11; energy candidate is native-only and unregistered",
        "binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"sources":{
            "solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),
            "longitudinal":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs")),
            "simulator":Digest::from_bytes(include_bytes!("../src/lib.rs")),
            "probe":Digest::from_bytes(include_bytes!("longitudinal_energy_probe.rs"))},
        "coarsening":factor,"cooling_scale":cooling,"engine_ambient_c":ambient,"competitors":results})
    );
    Ok(())
}

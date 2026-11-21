//! Native gameplay audit: existing equations, exact resolved career V9 inputs.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, resolve_career_physics_inputs_for_audit,
};
use pitgun_racing_solver::longitudinal_energy::LongitudinalEnergyModelV1;
use pitgun_racing_solver::{
    ResolvedSimulationSessionV3, ResolvedSimulationStepV3, SimulationSolution, TireParams,
    effective_mu,
};
use serde::Deserialize;
use serde_json::json;
use std::{env, fs};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Audit {
    request: RunRaceRequest,
    model: String,
    #[serde(default)]
    disable_derating: bool,
    #[serde(default)]
    disable_tire_temperature_feedback: bool,
    #[serde(default)]
    disable_tire_wear_grip: bool,
    initial_tire_temperature_c: Option<f64>,
    air_temperature_c: Option<f64>,
}
fn tire_override(t: &mut TireParams, a: &Audit) {
    if a.disable_tire_temperature_feedback {
        t.temp_min_k = 1.;
    }
    if a.disable_tire_wear_grip {
        t.wear_grip_k = 0.;
        t.wear_min = 1.;
    }
}
fn lap_summary(s: &SimulationSolution, tire: &TireParams) -> Result<serde_json::Value, String> {
    let n = s.t.len();
    if n < 2
        || [
            s.v.len(),
            s.temp.len(),
            s.tire_temp.len(),
            s.tire_wear.len(),
            s.engine_derating_factor.len(),
        ]
        .iter()
        .any(|l| *l != n)
        || s.t
            .windows(2)
            .any(|w| !w[0].is_finite() || !w[1].is_finite() || w[1] <= w[0])
    {
        return Err("inconsistent native lap arrays".into());
    }
    let duration = s.t[n - 1] - s.t[0];
    let (
        mut mean_t,
        mut floor,
        mut optimal,
        mut heating,
        mut cooling,
        mut mean_grip,
        mut derated,
        mut mean_factor,
    ) = (0., 0., 0., 0., 0., 0., 0., 0.);
    for i in 0..n - 1 {
        let dt = s.t[i + 1] - s.t[i];
        let temp = s.tire_temp[i];
        let factor = effective_mu(1., 0., temp, tire) / tire.mu_scale;
        mean_t += temp * dt;
        floor += if factor <= tire.temp_min_k + 1e-12 {
            dt
        } else {
            0.
        };
        optimal += if factor >= 0.95 { dt } else { 0. };
        heating += if s.tire_temp[i + 1] > temp + 1e-8 {
            dt
        } else {
            0.
        };
        cooling += if s.tire_temp[i + 1] < temp - 1e-8 {
            dt
        } else {
            0.
        };
        mean_grip += effective_mu(1., s.tire_wear[i], temp, tire) / tire.mu_scale * dt;
        derated += if s.engine_derating_factor[i] < 1. - 1e-12 {
            dt
        } else {
            0.
        };
        mean_factor += s.engine_derating_factor[i] * dt;
    }
    let extrema = |v: &[f64]| {
        (
            v.iter().copied().fold(f64::INFINITY, f64::min),
            v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    Ok(
        json!({"native_nodes":n,"duration_s":duration,"tire_initial_c":s.tire_temp[0],"tire_final_c":s.tire_temp[n-1],"tire_min_max_c":extrema(&s.tire_temp),"tire_mean_c":mean_t/duration,"temperature_factor_floor_time_fraction":floor/duration,"temperature_factor_at_least_95pct_time_fraction":optimal/duration,"heating_time_fraction":heating/duration,"cooling_time_fraction":cooling/duration,"mean_grip_factor_excluding_compound_scale":mean_grip/duration,"wear_initial":s.tire_wear[0],"wear_final":s.tire_wear[n-1],"engine_min_max_c":extrema(&s.temp),"derating_min_factor":extrema(&s.engine_derating_factor).0,"derating_mean_factor":mean_factor/duration,"observed_derated_time_s":derated,"tire":tire}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: thermal_tire_gameplay_probe CASE CATALOG".into());
    }
    let a: Audit = serde_json::from_slice(&fs::read(&args[0])?)?;
    if !["historical", "energy"].contains(&a.model.as_str())
        || !(1..=100).contains(&a.request.input.race.laps)
        || a.request.input.race.competitors.len() != 1
    {
        return Err("require historical|energy,1..100laps,one car".into());
    }
    for t in [a.initial_tire_temperature_c, a.air_temperature_c]
        .into_iter()
        .flatten()
    {
        if !t.is_finite() || !(0.0..=140.).contains(&t) {
            return Err("temperature audit bounds0..140C".into());
        }
    }
    let catalog = RacingCatalogSnapshot::from_release_dir(&args[1])?;
    let mut inputs = resolve_career_physics_inputs_for_audit(a.request.clone(), &catalog)?;
    let mut input = inputs.remove("player").ok_or("require player id")?;
    let original_input_digest = canonical_json_digest(&input)?;
    input.longitudinal_energy = (a.model == "energy").then_some(LongitudinalEnergyModelV1::V1);
    if a.disable_derating {
        let t = input.engine_thermal.get_or_insert_with(Default::default);
        t.minimum_power_fraction = 1.;
    }
    if a.disable_tire_temperature_feedback {
        input
            .tire_degradation
            .as_mut()
            .ok_or("require aggregate tire V2")?
            .thermal_deviation_wear_gain = 0.;
    }
    tire_override(&mut input.vehicle.tire, &a);
    for stop in &mut input.pit_plan.stops {
        tire_override(&mut stop.tire, &a);
    }
    if let Some(t) = a.initial_tire_temperature_c {
        input.state.tire_temp = t;
    }
    if let Some(t) = a.air_temperature_c {
        input.vehicle.engine.t_amb = t;
        input.config.tire_temp_amb = t;
    }
    let input_digest = canonical_json_digest(&input)?;
    let resolved = json!({"engine":input.vehicle.engine,"mechanical":input.mechanical,"contact":input.tire_contact,"degradation":input.tire_degradation,"driver":input.driver_control,"engine_thermal":input.engine_thermal,"initial_state":input.state,"tire_ambient_c":input.config.tire_temp_amb,"initial_tire":input.vehicle.tire});
    let mut tire = input.vehicle.tire.clone();
    let mut session = ResolvedSimulationSessionV3::new(input.clone())?;
    let mut laps = Vec::new();
    let outcome = loop {
        match session.advance() {
            Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                let mut stats = lap_summary(&lap.solution, &tire)?;
                stats["lap"] = json!(lap.lap_index);
                stats["after_service"] = json!(lap.state_after_lap);
                stats["pit_stop_completed"] = json!(lap.pit_stop_completed);
                laps.push(stats);
                if lap.pit_stop_completed {
                    tire = input
                        .pit_plan
                        .stops
                        .iter()
                        .find(|s| s.lap == lap.lap_index)
                        .ok_or("missing stop")?
                        .tire
                        .clone();
                }
            }
            Ok(ResolvedSimulationStepV3::Complete(r)) => {
                break json!({"ok":true,"result_digest":canonical_json_digest(&r)?,"total_time_s":r.total_time_s,"lap_times_s":r.lap_times_s,"final_state":r.final_state,"mechanical":r.mechanical_diagnostics_v3,"tire_diagnostics":r.tire_diagnostics_v3,"degradation":r.tire_degradation_diagnostics_v3,"fuel":r.fuel_mass_diagnostics_v3,"energy":r.longitudinal_energy_v1});
            }
            Err(e) => break json!({"ok":false,"error":e,"completed_observed_laps":laps.len()}),
        }
    };
    println!(
        "{}",
        json!({"schema":"pitgun.thermal-tire-gameplay-probe/v1","model":a.model,"request_digest":canonical_json_digest(&a.request)?,"original_resolved_input_digest":original_input_digest,"applied_input_digest":input_digest,"binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"catalog":catalog.release_identity(),"sources":{"solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),"energy":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs")),"simulator":Digest::from_bytes(include_bytes!("../src/lib.rs")),"probe":Digest::from_bytes(include_bytes!("thermal_tire_gameplay_probe.rs"))},"resolved":resolved,"laps":laps,"outcome":outcome,"diagnostic_overrides":{"disable_derating":a.disable_derating,"disable_tire_temperature_feedback":a.disable_tire_temperature_feedback,"disable_tire_wear_grip":a.disable_tire_wear_grip,"initial_tire_temperature_c":a.initial_tire_temperature_c,"air_temperature_c":a.air_temperature_c},"scope":"native career-V9 legacy and optional energy candidate; one aggregate patch, no deployed-byte or experimental-physics validation; diagnostic ablations not playable settings"})
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lap_metrics_use_time_weights_and_distinguish_cooling_from_service() {
        let s = SimulationSolution {
            t: vec![0., 1., 4.],
            v: vec![1.; 3],
            temp: vec![90.; 3],
            tire_temp: vec![90., 80., 85.],
            tire_wear: vec![0., 0.1, 0.2],
            engine_derating_factor: vec![1., 0.5, 0.5],
            ..Default::default()
        };
        let tire = TireParams {
            mu_scale: 1.,
            wear_per_s: 0.,
            wear_load_k: 0.,
            wear_grip_k: 0.5,
            wear_min: 0.5,
            temp_opt: 100.,
            temp_sigma: 45.,
            temp_min_k: 0.85,
            heat_k: 0.,
            cool_k: 0.,
        };
        let r = lap_summary(&s, &tire).unwrap();
        assert_eq!(r["tire_mean_c"], 82.5);
        assert_eq!(r["cooling_time_fraction"], 0.25);
        assert_eq!(r["heating_time_fraction"], 0.75);
        assert_eq!(r["temperature_factor_floor_time_fraction"], 0.75);
        assert_eq!(r["derating_mean_factor"], 0.625);
    }
    #[test]
    fn invalid_native_lap_is_not_a_successful_diagnostic() {
        assert!(
            lap_summary(
                &SimulationSolution::default(),
                &TireParams {
                    mu_scale: 1.,
                    wear_per_s: 0.,
                    wear_load_k: 0.,
                    wear_grip_k: 0.5,
                    wear_min: 0.5,
                    temp_opt: 100.,
                    temp_sigma: 45.,
                    temp_min_k: 0.85,
                    heat_k: 0.,
                    cool_k: 0.
                }
            )
            .is_err()
        );
    }
}

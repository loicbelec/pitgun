//! Local, unadmitted thermal candidate. No catalog or career runtime changes.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, resolve_career_physics_inputs_for_audit,
};
use pitgun_racing_solver::longitudinal_energy::LongitudinalEnergyModelV1;
use pitgun_racing_solver::thermal_physics::*;
use pitgun_racing_solver::{
    EngineThermalParamsV3, ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeMap, env, fs, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    request: RunRaceRequest,
    candidate: bool,
    initial_tire_temperature_c: Option<f64>,
    air_temperature_c: Option<f64>,
    road_temperature_c: Option<f64>,
    #[serde(default)]
    disable_derating: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    status: String,
    engine: EngineHeatParamsV2,
    radiator_speed_conductance_by_power_unit: BTreeMap<String, f64>,
    cooling_size_without_development: f64,
    cooling_size_gain_at_cap: f64,
    cooling_drag_area_m2_at_cap: f64,
    road_temperature_c: f64,
    compounds: BTreeMap<String, Compound>,
    note: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Compound {
    heat: TireHeatParamsV2,
    grip_scale: f64,
    temperature_optimum_c: f64,
    temperature_width_c: f64,
    minimum_temperature_grip_factor: f64,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: thermal_physics_v2_probe CASE CATALOG PROFILE".into());
    }
    let a: Case = serde_json::from_slice(&fs::read(&args[0])?)?;
    let p: Profile = serde_json::from_slice(&fs::read(&args[2])?)?;
    if a.request.input.race.competitors.len() != 1 {
        return Err("single-car audit only".into());
    }
    for t in [
        a.initial_tire_temperature_c,
        a.air_temperature_c,
        a.road_temperature_c,
    ]
    .into_iter()
    .flatten()
    {
        if !t.is_finite() || !(0.0..=140.0).contains(&t) {
            return Err("temperature diagnostic bounds0..140C".into());
        }
    }
    let catalog = RacingCatalogSnapshot::from_release_dir(&args[1])?;
    let mut inputs = resolve_career_physics_inputs_for_audit(a.request.clone(), &catalog)?;
    let mut input = inputs.remove("player").ok_or("player required")?;
    let original_digest = canonical_json_digest(&input)?;
    input.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
    if let Some(t) = a.initial_tire_temperature_c {
        input.state.tire_temp = t;
    }
    if let Some(t) = a.air_temperature_c {
        input.vehicle.engine.t_amb = t;
        input.config.tire_temp_amb = t;
    }
    if a.candidate {
        let raw = serde_json::to_value(&a.request)?;
        let competitor = &raw["input"]["competitors"][0];
        let vehicle_id = raw["input"]["vehicle_id"]
            .as_str()
            .ok_or("vehicle_id required")?;
        let base = Path::new(&args[1]).join("simulation");
        let vehicle: serde_json::Value = serde_json::from_slice(&fs::read(
            base.join("vehicles").join(format!("{vehicle_id}.json")),
        )?)?;
        let engine: serde_json::Value =
            serde_json::from_slice(&fs::read(base.join("engines").join(format!(
                "{}.json",
                vehicle["engine"].as_str().ok_or("engine id")?
            )))?)?;
        let thermal = &engine["thermal"];
        input.vehicle.engine.c_th = thermal["c_th"].as_f64().ok_or("c_th")?;
        input.vehicle.engine.t_soft = thermal["t_soft"].as_f64().ok_or("t_soft")?;
        input.vehicle.engine.beta_derate = thermal["beta_derate"].as_f64().ok_or("beta_derate")?;
        input.engine_thermal = Some(EngineThermalParamsV3::default());
        let cooling = competitor["tuning"]["cooling_points"]
            .as_f64()
            .ok_or("cooling points")?
            / 20.0;
        let sizing = p.cooling_size_without_development + p.cooling_size_gain_at_cap * cooling;
        let mut heat = p.engine;
        heat.radiator_idle_conductance_w_per_k *= sizing;
        heat.radiator_speed_conductance_w_per_k_mps = *p
            .radiator_speed_conductance_by_power_unit
            .get(vehicle["engine"].as_str().unwrap())
            .ok_or("missing installed engine radiator")?
            * sizing;
        // Replace only the old family's incremental cooling drag; body/aero remain resolved.
        let old_drag = if vehicle["engine"] == "v8_1960" || vehicle["engine"] == "v8_1970" {
            0.0
        } else {
            0.03633217993079584
        };
        input.mechanical.fixed_drag_area_m2 += (p.cooling_drag_area_m2_at_cap - old_drag) * cooling;
        let stints = competitor["stint_strategy"]["stints"]
            .as_array()
            .ok_or("explicit stints required")?;
        let mut compounds = Vec::new();
        let mut change =
            |tire: &mut pitgun_racing_solver::TireParams, id: &str| -> Result<(), String> {
                let c = p
                    .compounds
                    .get(id)
                    .ok_or_else(|| format!("unresolved thermal compound {id}"))?;
                tire.mu_scale = c.grip_scale;
                tire.temp_opt = c.temperature_optimum_c;
                tire.temp_sigma = c.temperature_width_c;
                tire.temp_min_k = c.minimum_temperature_grip_factor;
                if !compounds
                    .iter()
                    .any(|p: &TireCompoundHeatV2| p.tire == *tire)
                {
                    compounds.push(TireCompoundHeatV2 {
                        tire: tire.clone(),
                        heat: c.heat,
                    });
                }
                Ok(())
            };
        change(
            &mut input.vehicle.tire,
            stints[0]["tire_id"].as_str().ok_or("first tire")?,
        )?;
        for (stop, stint) in input.pit_plan.stops.iter_mut().zip(stints.iter().skip(1)) {
            change(&mut stop.tire, stint["tire_id"].as_str().ok_or("pit tire")?)?;
        }
        input.thermal_physics_v2 = Some(ThermalPhysicsV2 {
            schema_version: ThermalModelV2::V2,
            engine: heat,
            compounds,
            initial_carcass_temperature_c: input.state.tire_temp,
            pit_carcass_temperature_c: input.config.pit_tire_temp.unwrap_or(input.state.tire_temp),
            road_temperature_c: a.road_temperature_c.unwrap_or(p.road_temperature_c),
        });
    }
    if a.disable_derating {
        input
            .engine_thermal
            .get_or_insert_with(Default::default)
            .minimum_power_fraction = 1.0;
    }
    let mut session = ResolvedSimulationSessionV3::new(input.clone())?;
    let mut laps = Vec::new();
    let outcome = loop {
        match session.advance() {
            Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                if usize::from(lap.lap_index) != laps.len() + 1 {
                    return Err("nonsequential physical lap boundary".into());
                }
                let s = &lap.solution;
                let extrema = |v: &[f64]| {
                    (
                        v.iter().copied().fold(f64::INFINITY, f64::min),
                        v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    )
                };
                let derated =
                    s.t.windows(2)
                        .zip(&s.engine_derating_factor)
                        .filter(|(_, factor)| **factor < 1.0 - 1e-12)
                        .map(|(w, _)| w[1] - w[0])
                        .sum::<f64>();
                let heating =
                    s.t.windows(2)
                        .zip(s.tire_temp.windows(2))
                        .filter(|(_, w)| w[1] > w[0] + 1e-8)
                        .map(|(t, _)| t[1] - t[0])
                        .sum::<f64>();
                let cooling =
                    s.t.windows(2)
                        .zip(s.tire_temp.windows(2))
                        .filter(|(_, w)| w[1] < w[0] - 1e-8)
                        .map(|(t, _)| t[1] - t[0])
                        .sum::<f64>();
                laps.push(json!({"lap":lap.lap_index,"time_s":lap.lap_time_s,"engine_min_max_c":extrema(&s.temp),"derated_time_s":derated,"minimum_power_fraction":extrema(&s.engine_derating_factor).0,"surface_min_max_c":extrema(&s.tire_temp),"surface_end_c":s.tire_temp.last(),"carcass_end_c":s.tire_carcass_temp.last(),"surface_heating_time_s":heating,"surface_cooling_time_s":cooling,"wear":s.tire_wear.last(),"after_service":lap.state_after_lap,"thermal":lap.thermal_v2}));
            }
            Ok(ResolvedSimulationStepV3::Complete(r)) => {
                break json!({"ok":true,"digest":canonical_json_digest(&r)?,"time_s":r.total_time_s,"final_state":r.final_state,"energy":r.longitudinal_energy_v1,"thermal":r.thermal_v2});
            }
            Err(e) => break json!({"ok":false,"error":e}),
        }
    };
    println!(
        "{}",
        json!({"schema":"pitgun.thermal-physics-probe/v2","candidate":a.candidate,"catalog":catalog.release_identity(),"original_input_digest":original_digest,"input_digest":canonical_json_digest(&input)?,"resolved_parameters":{"vehicle":input.vehicle,"mechanical":input.mechanical,"contact":input.tire_contact,"fuel_mass":input.fuel_mass,"thermal_physics_v2":input.thermal_physics_v2,"initial_state":input.state,"driver_control":input.driver_control,"config":input.config},"profile_digest":Digest::from_bytes(&fs::read(&args[2])?),"profile_status":p.status,"profile_note":p.note,"compiled_sources":{"solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),"energy":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs")),"thermal":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/thermal_physics.rs")),"simulator":Digest::from_bytes(include_bytes!("../src/lib.rs")),"probe":Digest::from_bytes(include_bytes!("thermal_physics_v2_probe.rs"))},"binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"laps":laps,"outcome":outcome})
    );
    Ok(())
}

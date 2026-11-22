use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, resolve_career_physics_inputs_for_audit,
};
use pitgun_racing_solver::{
    ResolvedSimulationRequestV3, longitudinal_energy::LongitudinalEnergyModelV1,
};
use serde_json::{Value, json};
use std::{fs, path::Path};
pub fn frozen_input(
    root: &Path,
    id: &str,
    catalog: &RacingCatalogSnapshot,
) -> Result<(ResolvedSimulationRequestV3, Value), Box<dyn std::error::Error>> {
    let exp = root.join("experiments/thermal_physics_v2");
    let frozen: Value = serde_json::from_slice(&fs::read(exp.join("results/screen-v1.json"))?)?;
    let plan: Value = serde_json::from_slice(&fs::read(exp.join("plan-v1.json"))?)?;
    let params: Value = serde_json::from_slice(&fs::read(exp.join("profile-v1.json"))?)?;
    let case = plan["cases"]
        .as_array()
        .ok_or("missing cases")?
        .iter()
        .find(|c| c["id"] == id)
        .ok_or("missing frozen case")?;
    let baseline = &frozen["results"]
        .as_array()
        .ok_or("missing results")?
        .iter()
        .find(|c| c["case"] == id && c["candidate"] == true)
        .ok_or("missing frozen result")?["data"];
    let request: RunRaceRequest = serde_json::from_value(case["request"].clone())?;
    let mut input = resolve_career_physics_inputs_for_audit(request, catalog)?
        .remove("player")
        .ok_or("missing player")?;
    let p = &baseline["resolved_parameters"];
    input.vehicle = serde_json::from_value(p["vehicle"].clone())?;
    input.mechanical = serde_json::from_value(p["mechanical"].clone())?;
    input.tire_contact = serde_json::from_value(p["contact"].clone())?;
    input.fuel_mass = serde_json::from_value(p["fuel_mass"].clone())?;
    input.state = serde_json::from_value(p["initial_state"].clone())?;
    input.config = serde_json::from_value(p["config"].clone())?;
    input.driver_control = serde_json::from_value(p["driver_control"].clone())?;
    input.thermal_physics_v2 = serde_json::from_value(p["thermal_physics_v2"].clone())?;
    input.engine_thermal = Some(Default::default());
    input.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
    let stints = case["request"]["input"]["competitors"][0]["stint_strategy"]["stints"]
        .as_array()
        .ok_or("missing stints")?;
    for (stop, stint) in input.pit_plan.stops.iter_mut().zip(stints.iter().skip(1)) {
        let c = &params["compounds"][stint["tire_id"].as_str().ok_or("missing tire id")?];
        stop.tire.mu_scale = c["grip_scale"].as_f64().ok_or("grip")?;
        stop.tire.temp_opt = c["temperature_optimum_c"].as_f64().ok_or("opt")?;
        stop.tire.temp_sigma = c["temperature_width_c"].as_f64().ok_or("width")?;
        stop.tire.temp_min_k = c["minimum_temperature_grip_factor"]
            .as_f64()
            .ok_or("floor")?;
    }
    if json!(canonical_json_digest(&input)?) != baseline["input_digest"] {
        return Err(format!("frozen input differs: {id}").into());
    }
    Ok((input, baseline["outcome"].clone()))
}

//! Native-only qualification; frozen physical inputs, explicit driver policy.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::{RacingDriverTraitsV1, RacingDrivingMode};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, resolve_career_physics_inputs_for_audit,
    thermal_driver::{
        ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_resource_v1,
    },
};
use pitgun_racing_solver::{
    ResolvedServiceStepV3, ResolvedSimulationRequestV3, ResolvedSimulationSessionV3,
    ResolvedSimulationStepV3, SimulationResult, longitudinal_energy::LongitudinalEnergyModelV1,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{env, fs, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    frozen_case: String,
    #[serde(default)]
    baseline: bool,
    driver_id: Option<String>,
    mode: Option<RacingDrivingMode>,
    traits: Option<RacingDriverTraitsV1>,
    seed: Option<u64>,
    #[serde(default)]
    risk_off: bool,
    #[serde(default)]
    compare_service: bool,
}
fn frozen_input(
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
fn execute(input: &ResolvedSimulationRequestV3, service: bool) -> Result<SimulationResult, String> {
    let mut session = if service {
        ResolvedSimulationSessionV3::new_with_service_boundaries(input.clone())?
    } else {
        ResolvedSimulationSessionV3::new(input.clone())?
    };
    loop {
        if service {
            match session.advance_service()? {
                ResolvedServiceStepV3::BeforeService(_) => {
                    session.resolve_service_boundary(None)?;
                }
                ResolvedServiceStepV3::Lap(_) => {}
                ResolvedServiceStepV3::Complete(r) => return Ok(*r),
            }
        } else {
            match session.advance()? {
                ResolvedSimulationStepV3::Lap(_) => {}
                ResolvedSimulationStepV3::Complete(r) => return Ok(*r),
            }
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: thermal_driver_probe ROOT CASE POLICY".into());
    }
    let root = Path::new(&args[0]);
    let case: Case = serde_json::from_slice(&fs::read(&args[1])?)?;
    let policy: ThermalDriverPolicyV1 = serde_json::from_slice(&fs::read(&args[2])?)?;
    policy.validate()?;
    let catalog = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
    let (mut input, frozen_outcome) = frozen_input(root, &case.frozen_case, &catalog)?;
    let frozen_input_digest = canonical_json_digest(&input)?;
    let resolution = if case.baseline {
        if case.driver_id.is_some()
            || case.mode.is_some()
            || case.traits.is_some()
            || case.seed.is_some()
            || case.risk_off
        {
            return Err("baseline forbids driver/seed overrides".into());
        }
        None
    } else {
        let mut resource = catalog
            .drivers_v2()
            .get(case.driver_id.as_deref().ok_or("require driver")?)
            .ok_or("unknown V2 driver")?
            .clone();
        if let Some(traits) = case.traits {
            resource.traits = traits;
        }
        let mut resolution = resolve_thermal_driver_resource_v1(
            &resource,
            case.mode.ok_or("require mode")?,
            &policy,
            "thermal-driver:player",
        )?;
        // Diagnostic ablation, hashed into resolved input. Same actor/control noise.
        if case.risk_off {
            resolution.lapse_profile.hazard_per_100km = 0.0;
        }
        if let Some(seed) = case.seed {
            input.config.sim_seed = seed;
        }
        apply_thermal_driver_v1(&mut input, &resolution)?;
        Some(resolution)
    };
    let input_digest = canonical_json_digest(&input)?;
    let outcome = match execute(&input, false) {
        Ok(r) => {
            let digest = canonical_json_digest(&r)?;
            let service_equal = if case.compare_service {
                Some(r == execute(&input, true)?)
            } else {
                None
            };
            let s = &r.solution;
            json!({"ok":true,"digest":digest,"frozen_digest_equal":case.baseline.then(||json!(digest)==frozen_outcome["digest"]),"service_equal":service_equal,
                "time_s":r.total_time_s,"lap_times_s":r.lap_times_s,"final_state":r.final_state,
                "tire_temp_range_c":[s.tire_temp.iter().copied().fold(f64::INFINITY,f64::min),s.tire_temp.iter().copied().fold(f64::NEG_INFINITY,f64::max)],
                "mechanical":r.mechanical_diagnostics_v3,"tire":r.tire_diagnostics_v3,"wear":r.tire_degradation_diagnostics_v3,
                "energy":r.longitudinal_energy_v1,"lapses":s.driver_lapses_v1})
        }
        Err(error) => json!({"ok":false,"error":error}),
    };
    println!(
        "{}",
        json!({"schema":"pitgun.thermal-driver-probe/v1","id":case.id,"frozen_input_digest":frozen_input_digest,"input_digest":input_digest,"resolution":resolution,
        "binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"catalog":catalog.release_identity(),
        "sources":{"probe":Digest::from_bytes(include_bytes!("thermal_driver_probe.rs")),"driver":Digest::from_bytes(include_bytes!("../src/thermal_driver.rs")),"lapses":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/driver_lapses.rs")),"solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),"energy":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs"))},"outcome":outcome})
    );
    Ok(())
}

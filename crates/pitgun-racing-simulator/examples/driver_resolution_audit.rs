//! Read-only audit of the actual career-V9 resolution and separate V2 trait mapper.
//! No simulation, calibration, incident generation or runtime promotion.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::RacingDrivingMode;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, resolve_career_physics_inputs_for_audit,
    resolve_v3_driver_control_v1,
};
use serde_json::{Value, json};
use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 1 {
        return Err("usage: driver_resolution_audit REPOSITORY_ROOT".into());
    }
    let root = Path::new(&args[0]);
    let catalog = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
    let plan_bytes = fs::read(root.join("experiments/thermal_physics_v2/plan-v1.json"))?;
    let plan: Value = serde_json::from_slice(&plan_bytes)?;
    let request: RunRaceRequest = serde_json::from_value(plan["cases"][0]["request"].clone())?;
    let mut career = Vec::new();
    for id in [
        "default",
        "balanced_reference",
        "limit_specialist",
        "smooth_operator",
        "tire_manager",
        "franz_hermann",
        "battery_voltas",
        "not-a-driver",
    ] {
        let mut request = request.clone();
        request.input.race.competitors[0].driver_id = Some(id.into());
        let request_digest = canonical_json_digest(&request)?;
        let mut resolved = resolve_career_physics_inputs_for_audit(request, &catalog)?;
        let input = resolved.remove("player").ok_or("missing player")?;
        career.push(json!({"requested_driver_id":id,"request_digest":request_digest,
            "resolved_input_digest":canonical_json_digest(&input)?,"resolved_driver":input.driver,
            "resolved_controls":input.driver_control,"correction_workload_multiplier":input.driver_correction_workload_multiplier,
            "driver_control_schedule_length":input.driver_control_schedule.len()}));
    }
    let profile = catalog
        .driver_control_profile()
        .ok_or("missing V2 trait profile")?;
    let mut trait_mapping = Vec::new();
    for driver in catalog.drivers_v2().values() {
        for mode in [
            RacingDrivingMode::Manage,
            RacingDrivingMode::Balanced,
            RacingDrivingMode::Attack,
        ] {
            trait_mapping.push(resolve_v3_driver_control_v1(driver, mode, profile)?);
        }
    }
    println!(
        "{}",
        json!({"schema":"pitgun.driver-resolution-audit/v1","catalog":catalog.release_identity(),
        "source_plan_digest":Digest::from_bytes(&plan_bytes),"binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),
        "compiled_sources":{"simulator":Digest::from_bytes(include_bytes!("../src/lib.rs")),
            "solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),
            "catalog":Digest::from_bytes(include_bytes!("../src/catalog.rs")),
            "driver_contract":Digest::from_bytes(include_bytes!("../../pitgun-racing-contract/src/driver.rs")),
            "probe":Digest::from_bytes(include_bytes!("driver_resolution_audit.rs"))},
        "career_v9_resolution":career,"separate_v2_trait_mapping":trait_mapping,
        "simulation_executions":0,"scope":"Native career-V9 resolution and separate trait arithmetic only. No assertion about deployed requests, performance, incident probabilities, driver integration into thermal V2 or balance."})
    );
    Ok(())
}

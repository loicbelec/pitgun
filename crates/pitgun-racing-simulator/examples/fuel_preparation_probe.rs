//! Fixed native fuel screen; no hosted workload or browser activation.
#[path = "support/frozen_thermal_input.rs"]
mod frozen;
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::RacingDrivingMode;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot,
    fuel_preparation::{FuelPreparationPolicyV1, prepare_fuel_v1, run_fuel_aware_field_v1},
    thermal_driver::{ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_v1},
};
use pitgun_racing_solver::{
    ResolvedSimulationRequestV3,
    fuel_outcome::{FuelAwareSessionV1, FuelAwareStepV1},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, env, fs, path::Path};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    frozen_case: String,
    mode: RacingDrivingMode,
    #[serde(default)]
    compare_original: bool,
    #[serde(default)]
    isolation: bool,
    #[serde(default)]
    diagnostic_ceiling_only: bool,
}
fn execute(input: ResolvedSimulationRequestV3) -> Result<Value, String> {
    let mut session = FuelAwareSessionV1::new(input)?;
    let mut progress = Vec::new();
    loop {
        match session.advance()? {
            FuelAwareStepV1::Lap { progress: p } => progress.push(p),
            FuelAwareStepV1::Finished { result } => {
                return Ok(
                    json!({"kind":"finished","digest":canonical_json_digest(&result).map_err(|e|e.to_string())?,"time_s":result.total_time_s,"state":result.final_state,"laps":progress,"energy":result.longitudinal_energy_v1}),
                );
            }
            FuelAwareStepV1::FuelNotCompleted {
                progress: p,
                shortfall,
            } => {
                return Ok(
                    json!({"kind":"fuel_not_completed","progress":p,"shortfall":shortfall,"laps":progress}),
                );
            }
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: fuel_preparation_probe ROOT CASE POLICY".into());
    }
    let root = Path::new(&args[0]);
    let case: Case = serde_json::from_slice(&fs::read(&args[1])?)?;
    let policy: FuelPreparationPolicyV1 = serde_json::from_slice(&fs::read(&args[2])?)?;
    let catalog = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
    let (mut input, _) = frozen::frozen_input(root, &case.frozen_case, &catalog)?;
    let driver_policy: ThermalDriverPolicyV1 = serde_json::from_slice(&fs::read(
        root.join("experiments/thermal_driver_v1/profile-v1.json"),
    )?)?;
    let driver = resolve_thermal_driver_v1(
        &catalog,
        "balanced_reference",
        case.mode,
        &driver_policy,
        "fuel-preparation:player",
    )?;
    apply_thermal_driver_v1(&mut input, &driver)?;
    let original_digest = canonical_json_digest(&input)?;
    let original = if case.compare_original {
        Some(execute(input.clone()))
    } else {
        None
    };
    let prepared = if case.diagnostic_ceiling_only {
        input.state.fuel_mass = policy.maximum_start_kg;
        json!({"diagnostic_ceiling_kg":policy.maximum_start_kg,"diagnostic_input_digest":canonical_json_digest(&input)?,"outcome":execute(input.clone())})
    } else {
        match prepare_fuel_v1(&input, &policy) {
            Ok((receipt, resolved)) => {
                let outcome = resolved.clone().map(execute);
                let isolation = if case.isolation {
                    let healthy = resolved.ok_or("isolation requires prepared input")?;
                    let mut rival = healthy.clone();
                    rival.state.fuel_mass = 0.000001;
                    Some(run_fuel_aware_field_v1(BTreeMap::from([
                        ("player".into(), healthy),
                        ("rival".into(), rival),
                    ])))
                } else {
                    None
                };
                json!({"receipt":receipt,"outcome":outcome,"isolation":isolation})
            }
            Err(e) => json!({"technical_error":e}),
        }
    };
    println!(
        "{}",
        json!({"schema":"pitgun.fuel-preparation-probe/v1","id":case.id,"original_input_digest":original_digest,"catalog":catalog.release_identity(),"driver":driver,"original":original,"prepared":prepared,
        "binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"sources":{"probe":Digest::from_bytes(include_bytes!("fuel_preparation_probe.rs")),"helper":Digest::from_bytes(include_bytes!("support/frozen_thermal_input.rs")),"preparation":Digest::from_bytes(include_bytes!("../src/fuel_preparation.rs")),"outcome":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/fuel_outcome.rs")),"solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs"))}})
    );
    Ok(())
}

//! Local baseline screen only: no Authority, publication, or provider calls.

use std::{env, fs};

use pitgun_contract::{canonical_json_bytes, canonical_json_digest};
use pitgun_racing_contract::RacingDriverInstructionTimelineV1;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceInput, RunRaceRequest, evidence::RacingRunEvidenceV1,
    racing_model_v3_fuel_contract_candidate_identity,
    run_race_with_catalog_and_v3_fuel_contract_candidate,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    request: RunRaceInput,
    seed: u64,
    timeline: RacingDriverInstructionTimelineV1,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    if arguments.len() != 1 {
        return Err("usage: engineer_baseline_probe CASE_JSON".into());
    }
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(&arguments[0])?)?;
    let case: Case = serde_json::from_value(raw.clone())?;
    let catalog = RacingCatalogSnapshot::embedded_model_v3_fuel_contract()?;
    let output = run_race_with_catalog_and_v3_fuel_contract_candidate(
        RunRaceRequest {
            era: Some(case.request.era),
            hz: Some(case.request.hz),
            input: case.request,
            seed: case.seed,
        },
        &catalog,
        case.timeline,
    )?;
    let evidence = RacingRunEvidenceV1::from_race_output(&output)?;
    let result = json!({
        "schema_version": "pitgun.engineer-baseline-probe/v1",
        "case_digest": canonical_json_digest(&raw)?,
        "model": racing_model_v3_fuel_contract_candidate_identity(),
        "catalog": catalog.release_identity(),
        "output_digest": evidence.output_digest()?,
        "telemetry_summary_digest": evidence.telemetry_summary_digest()?,
        "output": evidence.output,
        "tire_diagnostics": output.player_tire_diagnostics_v3,
        "tire_degradation_diagnostics": output.player_tire_degradation_diagnostics_v3,
        "driver_control_diagnostics": output.competitor_driver_control_diagnostics_v3.get("player"),
    });
    println!("{}", String::from_utf8(canonical_json_bytes(&result)?)?);
    Ok(())
}

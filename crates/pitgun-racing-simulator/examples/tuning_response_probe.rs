//! Offline-only probe for bounded Racing tuning-response experiments.

use std::env;
use std::fs;
use std::path::Path;

use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceInput, RunRaceRequest, SetupResponseDiagnosticsV1,
    TuningResponseV1, run_race_with_catalog_and_tuning_response,
};
use serde::Serialize;
use serde_json::Value;

const PARAM_SPEED_KPH: u16 = 5005;

#[derive(Serialize)]
struct ExperimentalIdentity<'a> {
    schema_version: &'static str,
    scenario: &'a Value,
    tuning_response: &'a TuningResponseV1,
    seed: u64,
}

#[derive(Serialize)]
struct ProbeOutput {
    schema_version: &'static str,
    experimental_execution_id: String,
    scenario_digest: String,
    tuning_response_digest: String,
    seed: String,
    total_time_ms: u64,
    observed_maximum_speed_kph: f64,
    setup_response: SetupResponseDiagnosticsV1,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 {
        return Err(
            "usage: tuning_response_probe SCENARIO_JSON TUNING_RESPONSE_JSON SEED".to_string(),
        );
    }
    let seed = arguments[2]
        .parse::<u64>()
        .map_err(|error| format!("invalid seed: {error}"))?;
    let scenario = read_json(&arguments[0], "scenario")?;
    if scenario.get("schema_version").and_then(Value::as_str)
        != Some("pitgun.racing-resolved-scenario/v1")
    {
        return Err("unsupported Racing scenario version".to_string());
    }
    let input = serde_json::from_value::<RunRaceInput>(
        scenario
            .get("request")
            .cloned()
            .ok_or_else(|| "scenario request is missing".to_string())?,
    )
    .map_err(|error| format!("invalid Racing request: {error}"))?;
    let tuning_response =
        serde_json::from_value::<TuningResponseV1>(read_json(&arguments[1], "tuning response")?)
            .map_err(|error| format!("invalid tuning response: {error}"))?;
    tuning_response
        .validate()
        .map_err(|error| format!("invalid tuning response: {error}"))?;

    let scenario_digest = canonical_json_digest(&scenario)
        .map_err(|error| format!("cannot digest scenario: {error}"))?;
    let tuning_response_digest = canonical_json_digest(&tuning_response)
        .map_err(|error| format!("cannot digest tuning response: {error}"))?;
    let experimental_execution_id = canonical_json_digest(&ExperimentalIdentity {
        schema_version: "pitgun.racing-tuning-response-probe/v1",
        scenario: &scenario,
        tuning_response: &tuning_response,
        seed,
    })
    .map_err(|error| format!("cannot identify experimental execution: {error}"))?;

    let snapshot = RacingCatalogSnapshot::embedded()
        .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
    let output = run_race_with_catalog_and_tuning_response(
        RunRaceRequest {
            era: Some(input.era),
            hz: Some(input.hz),
            input,
            seed,
        },
        &snapshot,
        &tuning_response,
    )?;
    let observed_maximum_speed_kph = output
        .player_batches
        .iter()
        .flat_map(|batch| &batch.frames)
        .flat_map(|frame| &frame.samples)
        .filter(|sample| sample.parameter_id == PARAM_SPEED_KPH)
        .filter_map(|sample| sample.value.as_f64())
        .reduce(f64::max)
        .ok_or_else(|| "race output contains no player speed telemetry".to_string())?;
    let setup_response = output
        .player_diagnostics
        .ok_or_else(|| "race output contains no setup diagnostics".to_string())?;

    let probe = ProbeOutput {
        schema_version: "pitgun.racing-tuning-response-probe/v1",
        experimental_execution_id: experimental_execution_id.to_string(),
        scenario_digest: scenario_digest.to_string(),
        tuning_response_digest: tuning_response_digest.to_string(),
        seed: seed.to_string(),
        total_time_ms: output.total_time_ms,
        observed_maximum_speed_kph,
        setup_response,
    };
    println!(
        "{}",
        serde_json::to_string(&probe)
            .map_err(|error| format!("cannot serialize probe output: {error}"))?
    );
    Ok(())
}

fn read_json(path: impl AsRef<Path>, label: &str) -> Result<Value, String> {
    let path = path.as_ref();
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {label} JSON {}: {error}", path.display()))
}

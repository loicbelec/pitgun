//! Offline-only telemetry probe for locating Racing relief response by distance.

use std::env;
use std::fs;
use std::path::Path;

use pitgun_contract::{TelemetryFrame, canonical_json_digest};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceInput, RunRaceRequest, TuningResponseV1,
    run_race_with_catalog_and_tuning_response,
};
use serde::Serialize;
use serde_json::Value;

const PARAM_SPEED_KPH: u16 = 5005;
const PARAM_THROTTLE_PCT: u16 = 5008;
const PARAM_BRAKE_PCT: u16 = 5009;
const PARAM_G_LONG: u16 = 5011;
const PARAM_G_VERT: u16 = 5012;

#[derive(Serialize)]
struct ExperimentalIdentity<'a> {
    schema_version: &'static str,
    scenario: &'a Value,
    tuning_response: &'a TuningResponseV1,
    seed: u64,
}

#[derive(Serialize)]
struct TelemetryPoint {
    distance_m: f64,
    speed_kph: f64,
    throttle_pct: f64,
    brake_pct: f64,
    g_long: f64,
    g_vert: f64,
}

#[derive(Serialize)]
struct ProbeOutput {
    schema_version: &'static str,
    experimental_execution_id: String,
    scenario_digest: String,
    tuning_response_digest: String,
    seed: String,
    total_time_ms: u64,
    telemetry: Vec<TelemetryPoint>,
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
            "usage: relief_response_probe SCENARIO_JSON TUNING_RESPONSE_JSON SEED".to_string(),
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
        schema_version: "pitgun.racing-relief-response-probe/v1",
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
            hz: Some(20.0),
            input,
            seed,
        },
        &snapshot,
        &tuning_response,
    )?;
    let telemetry = output
        .player_batches
        .iter()
        .flat_map(|batch| &batch.frames)
        .map(telemetry_point)
        .collect::<Result<Vec<_>, _>>()?;
    if telemetry.len() < 2 {
        return Err("race output contains insufficient telemetry".to_string());
    }

    let probe = ProbeOutput {
        schema_version: "pitgun.racing-relief-response-probe/v1",
        experimental_execution_id: experimental_execution_id.to_string(),
        scenario_digest: scenario_digest.to_string(),
        tuning_response_digest: tuning_response_digest.to_string(),
        seed: seed.to_string(),
        total_time_ms: output.total_time_ms,
        telemetry,
    };
    println!(
        "{}",
        serde_json::to_string(&probe)
            .map_err(|error| format!("cannot serialize probe output: {error}"))?
    );
    Ok(())
}

fn telemetry_point(frame: &TelemetryFrame) -> Result<TelemetryPoint, String> {
    Ok(TelemetryPoint {
        distance_m: frame
            .progress_m
            .map(f64::from)
            .ok_or_else(|| "telemetry frame contains no distance".to_string())?,
        speed_kph: sample(frame, PARAM_SPEED_KPH)?,
        throttle_pct: sample(frame, PARAM_THROTTLE_PCT)?,
        brake_pct: sample(frame, PARAM_BRAKE_PCT)?,
        g_long: sample(frame, PARAM_G_LONG)?,
        g_vert: sample(frame, PARAM_G_VERT)?,
    })
}

fn sample(frame: &TelemetryFrame, parameter_id: u16) -> Result<f64, String> {
    frame
        .samples
        .iter()
        .find(|sample| sample.parameter_id == parameter_id)
        .and_then(|sample| sample.value.as_f64())
        .ok_or_else(|| format!("telemetry frame contains no parameter {parameter_id}"))
}

fn read_json(path: impl AsRef<Path>, label: &str) -> Result<Value, String> {
    let path = path.as_ref();
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {label} JSON {}: {error}", path.display()))
}

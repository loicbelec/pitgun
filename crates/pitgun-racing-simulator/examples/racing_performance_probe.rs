//! Isolated release-mode diagnostic; no provider, Authority or browser calls.
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, V3PowerUnitThermalProfileCandidateV2,
    complete_local_racing_session_json, pull_local_racing_session_json,
    run_race_with_catalog_and_v3_power_unit_thermal_profile,
    start_local_racing_session_with_catalog_and_v3_power_unit_thermal_profile_json,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{env, fs, time::Instant};

#[derive(Deserialize)]
struct Input {
    request_json: String,
    catalog_bundle_json: String,
    profile_json: String,
}

fn milliseconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn parse(raw: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if value.get("error").is_some() {
        return Err(value.to_string());
    }
    Ok(value)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        return Err("MODE INPUT_JSON REPORT_JSON; MODE is typed or json".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut api_ms = Vec::new();
    let mut parse_ms = 0.0;
    let mut stream_bytes: usize = 0;
    let started = Instant::now();
    let (result, setup_ms, finish_ms, compute_ms, frames, completion_bytes) = match args[1].as_str()
    {
        "typed" => {
            let request: RunRaceRequest =
                serde_json::from_str(&input.request_json).map_err(|e| e.to_string())?;
            let catalog = RacingCatalogSnapshot::from_bundle_json(&input.catalog_bundle_json)
                .map_err(|e| e.to_string())?;
            let profile =
                V3PowerUnitThermalProfileCandidateV2::from_exact_json(&input.profile_json)?;
            let setup_ms = milliseconds(started);
            let begin = Instant::now();
            let output = run_race_with_catalog_and_v3_power_unit_thermal_profile(
                request, &catalog, &profile,
            )?;
            let compute_ms = milliseconds(begin);
            // Keep only small validation summaries. Converting the entire
            // typed result to serde_json::Value would contaminate its RSS peak.
            let begin = Instant::now();
            let frames: usize = output.player_batches.iter().map(|b| b.frames.len()).sum();
            let result = json!({"standings":output.standings,
                "player_lap_times_ms":output.player_lap_times_ms});
            (result, setup_ms, milliseconds(begin), compute_ms, frames, 0)
        }
        "json" => {
            let begin = Instant::now();
            let raw =
                start_local_racing_session_with_catalog_and_v3_power_unit_thermal_profile_json(
                    input.request_json,
                    input.catalog_bundle_json,
                    input.profile_json,
                );
            let setup_ms = milliseconds(begin);
            let begin = Instant::now();
            let start = parse(&raw)?;
            parse_ms += milliseconds(begin);
            let handle = start["handle"].as_u64().ok_or("no handle")? as u32;
            let compute_begin = Instant::now();
            loop {
                let begin = Instant::now();
                let raw = pull_local_racing_session_json(handle);
                api_ms.push(milliseconds(begin));
                stream_bytes += raw.len();
                let begin = Instant::now();
                let pulled = parse(&raw)?;
                parse_ms += milliseconds(begin);
                if pulled["complete"] == true {
                    break;
                }
            }
            let compute_ms = milliseconds(compute_begin);
            let begin = Instant::now();
            let raw = complete_local_racing_session_json(handle);
            let finish_ms = milliseconds(begin);
            let completion_bytes = raw.len();
            let begin = Instant::now();
            let mut completed = parse(&raw)?;
            parse_ms += milliseconds(begin);
            let frames: usize = completed["result"]["player_batches"]
                .as_array()
                .ok_or("missing telemetry")?
                .iter()
                .map(|batch| batch["frames"].as_array().map_or(0, Vec::len))
                .sum();
            (
                completed["result"].take(),
                setup_ms,
                finish_ms,
                compute_ms,
                frames,
                completion_bytes,
            )
        }
        _ => return Err("unknown mode".into()),
    };
    let elapsed_ms = milliseconds(started);
    let report = json!({"runtime":"native","mode":args[1],"setup_ms":setup_ms,
        "compute_ms":compute_ms,"finish_ms":finish_ms,"host_parse_ms":parse_ms,
        "elapsed_ms":elapsed_ms,"pull_api_ms":api_ms,"stream_bytes":stream_bytes,
        "completion_bytes":completion_bytes,"frames":frames,"standings":result["standings"],
        "player_lap_times_ms":result["player_lap_times_ms"],
        "notes":"RSS includes JSON host parse where applicable; typed mode avoids full JSON validation conversion; no UI or browser qualification"});
    fs::write(
        &args[3],
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

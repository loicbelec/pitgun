//! Offline replay of a previously authorised request. Never submits evidence.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{
    complete_authorized_dynamic_racing_session_json, pull_authorized_dynamic_racing_session_json,
    start_authorized_dynamic_racing_session_with_catalog_json,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{env, fs, process::Command, time::Instant};

#[derive(Deserialize)]
struct Input {
    request_json: String,
    catalog_bundle_json: String,
}
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn parse(raw: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if value.get("error").is_some() {
        return Err(value.to_string());
    }
    Ok(value)
}
fn rss(phase: &str) -> Value {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output();
    let bytes = output
        .ok()
        .filter(|r| r.status.success())
        .and_then(|r| String::from_utf8(r.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|n| n * 1024);
    json!({"phase":phase,"rss_bytes":bytes})
}
fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 || args.len() > 5 {
        return Err("measure|dump|canonical INPUT REPORT [RAW_COMPLETION]".into());
    }
    if args[1] == "canonical" {
        let value: Value = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let (whole, evidence, output) = if value.get("result").is_some() {
            (
                &value["result"],
                &value["result"]["evidence"],
                &value["result"]["runtime_output"],
            )
        } else {
            (&value, &value, &Value::Null)
        };
        let report = json!({"application_digest":canonical_json_digest(whole).map_err(|e|e.to_string())?,
            "evidence_digest":canonical_json_digest(evidence).map_err(|e|e.to_string())?,
            "runtime_output_digest":canonical_json_digest(output).map_err(|e|e.to_string())?});
        return fs::write(
            &args[3],
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string());
    }
    if args[1] != "measure" && args[1] != "dump" {
        return Err("unsupported mode".into());
    }
    if args[1] == "dump" && args.len() != 5 {
        return Err("dump requires a private raw-output path".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let request: Value = serde_json::from_str(&input.request_json).map_err(|e| e.to_string())?;
    let laps = request["input"]["laps"].as_u64().ok_or("missing laps")? as usize;
    let mut memory = vec![rss("before_start")];
    let begin = Instant::now();
    let raw = start_authorized_dynamic_racing_session_with_catalog_json(
        input.request_json,
        input.catalog_bundle_json,
    );
    let setup_ms = ms(begin);
    let begin = Instant::now();
    let start = parse(&raw)?;
    let mut host_parse_ms = ms(begin);
    let handle = start["handle"].as_u64().ok_or("missing handle")? as u32;
    memory.push(rss("after_setup"));
    let mut pull_api_ms = Vec::new();
    let mut stream_bytes = 0_usize;
    loop {
        let begin = Instant::now();
        let raw = pull_authorized_dynamic_racing_session_json(handle);
        pull_api_ms.push(ms(begin));
        stream_bytes += raw.len();
        let begin = Instant::now();
        let batch = parse(&raw)?;
        host_parse_ms += ms(begin);
        if pull_api_ms.len() == laps {
            memory.push(rss("after_last_lap_before_finalization"));
        }
        if batch["complete"] == true {
            memory.push(rss("after_terminal_pull"));
            break;
        }
    }
    let begin = Instant::now();
    let raw = complete_authorized_dynamic_racing_session_json(handle);
    let completion_ms = ms(begin);
    let completion_bytes = raw.len();
    let begin = Instant::now();
    let completed = parse(&raw)?;
    host_parse_ms += ms(begin);
    memory.push(rss("after_completion"));
    let result = &completed["result"]["runtime_output"];
    let frames: usize = result["player_batches"]
        .as_array()
        .ok_or("missing runtime telemetry")?
        .iter()
        .map(|b| b["frames"].as_array().map_or(0, Vec::len))
        .sum();
    let report = json!({"runtime":"native","model":"0.14.0","setup_ms":setup_ms,
        "pull_api_ms":pull_api_ms,"completion_ms":completion_ms,"host_parse_ms":host_parse_ms,
        "stream_bytes":stream_bytes,"completion_bytes":completion_bytes,"memory_snapshots":memory,
        "frames":frames,"run_id":completed["result"]["evidence"]["receipt"]["receipt"]["run_id"],
        "standings":result["standings"],"player_lap_times_ms":result["player_lap_times_ms"]});
    fs::write(
        &args[3],
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if args[1] == "dump" {
        fs::write(&args[4], raw).map_err(|e| e.to_string())?;
    }
    Ok(())
}

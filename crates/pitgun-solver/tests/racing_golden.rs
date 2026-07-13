use pitgun_solver::{RaceOutput, run_race_json};
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const INPUT: &str = include_str!("golden/racing_run_v1.input.json");
const EXPECTED: &str = include_str!("golden/racing_run_v1.expected.json");

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct GoldenSummary {
    total_time_ms: u64,
    player_lap_times_ms: Vec<u64>,
    standings: Vec<GoldenStanding>,
    telemetry: GoldenTelemetry,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct GoldenStanding {
    competitor_id: String,
    position: u32,
    total_time_ms: u64,
    best_lap_ms: u64,
    laps_completed: u16,
    gap_to_leader_ms: u64,
    status: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct GoldenTelemetry {
    batch_count: usize,
    frame_count: usize,
    first_timestamp_us: i64,
    last_timestamp_us: i64,
    first_sequence: u64,
    last_sequence: u64,
    samples_per_frame: usize,
    parameter_ids: Vec<u16>,
    first_lap_number: Option<u16>,
    last_lap_number: Option<u16>,
    source_id: String,
    sampling_hz: String,
}

fn summarize(output: RaceOutput) -> GoldenSummary {
    let frames = output
        .player_batches
        .iter()
        .flat_map(|batch| batch.frames.iter())
        .collect::<Vec<_>>();
    let first = frames.first().expect("golden run must emit telemetry");
    let last = frames.last().expect("golden run must emit telemetry");

    GoldenSummary {
        total_time_ms: output.total_time_ms,
        player_lap_times_ms: output.player_lap_times_ms,
        standings: output
            .standings
            .into_iter()
            .map(|standing| GoldenStanding {
                competitor_id: standing.competitor_id,
                position: standing.position,
                total_time_ms: standing.total_time_ms,
                best_lap_ms: standing.best_lap_ms,
                laps_completed: standing.laps_completed,
                gap_to_leader_ms: standing.gap_to_leader_ms,
                status: serde_json::to_value(standing.status)
                    .expect("standing status must serialize")
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .expect("standing status must have a type")
                    .to_string(),
            })
            .collect(),
        telemetry: GoldenTelemetry {
            batch_count: output.player_batches.len(),
            frame_count: frames.len(),
            first_timestamp_us: first.timestamp_us,
            last_timestamp_us: last.timestamp_us,
            first_sequence: first.sequence,
            last_sequence: last.sequence,
            samples_per_frame: first.samples.len(),
            parameter_ids: first
                .samples
                .iter()
                .map(|sample| sample.parameter_id)
                .collect(),
            first_lap_number: first.lap_number,
            last_lap_number: last.lap_number,
            source_id: first.source_id.clone(),
            sampling_hz: first
                .metadata
                .get("sampling_hz")
                .expect("sampling_hz metadata")
                .clone(),
        },
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v1_matches_the_versioned_golden_summary() {
    let response = run_race_json(INPUT.to_string());
    let output: RaceOutput = serde_json::from_str(&response)
        .unwrap_or_else(|error| panic!("golden run returned invalid output: {error}: {response}"));
    let actual = summarize(output);
    let expected: GoldenSummary =
        serde_json::from_str(EXPECTED).expect("golden summary fixture must be valid");

    assert_eq!(
        actual,
        expected,
        "Racing golden run changed. Update the model or contract version before accepting a new fixture.\nActual summary:\n{}",
        serde_json::to_string_pretty(&actual).expect("golden summary must serialize")
    );
}

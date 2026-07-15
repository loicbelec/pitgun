use std::fmt::Debug;

use pitgun_contract::{
    DeterministicRunContractV1, Digest, Identifier, RuntimeIdentity, Seed, SemanticVersion,
    canonical_json_bytes, canonical_json_digest, canonicalize_json_str,
};
use pitgun_solver::evidence::RacingRunEvidenceV1;
use pitgun_solver::{RaceOutput, run_race_json};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const INPUT: &str = include_str!("golden/racing_run_v1.input.json");
const EXPECTED: &str = include_str!("golden/racing_run_v1.expected.json");
const CONTRACT: &str = include_str!("golden/racing_run_v1.contract.json");
const EXPECTED_OUTPUT: &str = include_str!("golden/racing_run_v1.output.json");
const EXPECTED_TELEMETRY_SUMMARY: &str =
    include_str!("golden/racing_run_v1.telemetry-summary.json");
const EXPECTED_DIGESTS: &str = include_str!("golden/racing_run_v1.digests.json");
const MODEL_IDENTITY: &str = "pitgun.racing:model:1.0.0:conformance-vector";
const DATA_PACK_IDENTITY: &str = "pitgun.racing.2026:data-pack:1.0.0:conformance-vector";

#[cfg(target_arch = "wasm32")]
const TARGET: &str = "wasm32-unknown-unknown";
#[cfg(not(target_arch = "wasm32"))]
const TARGET: &str = "native-test-target";

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

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct GoldenDigests {
    run_id: Digest,
    output_digest: Digest,
    telemetry_summary_digest: Digest,
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
    let output = run_golden_race();
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v1_matches_published_canonical_artifacts_and_digests() {
    let output = run_golden_race();
    let evidence = RacingRunEvidenceV1::from_race_output(&output).expect("Racing evidence");
    let contract: DeterministicRunContractV1 =
        serde_json::from_str(CONTRACT).expect("Racing deterministic contract");

    assert_artifact_eq("Racing output", &evidence.output, EXPECTED_OUTPUT);
    assert_artifact_eq(
        "telemetry summary",
        &evidence.telemetry_summary,
        EXPECTED_TELEMETRY_SUMMARY,
    );

    let canonical_input = canonicalize_json_str(INPUT).expect("canonical Racing input");
    let input_digest = Digest::from_bytes(&canonical_input);
    assert_eq!(
        contract.input.digest, input_digest,
        "contract input.digest must bind the canonical Racing input"
    );
    assert_eq!(
        contract.model.digest,
        Digest::from_bytes(MODEL_IDENTITY.as_bytes()),
        "contract model.digest must bind the published conformance identity"
    );
    assert_eq!(
        contract.data_pack.digest,
        Digest::from_bytes(DATA_PACK_IDENTITY.as_bytes()),
        "contract data_pack.digest must bind the published conformance identity"
    );

    let execution_id = "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
        .parse()
        .expect("execution id");
    let receipt = evidence
        .execution_receipt(
            &contract,
            execution_id,
            RuntimeIdentity {
                engine: Identifier::new("pitgun-rust").expect("engine id"),
                engine_version: SemanticVersion::new("0.1.0").expect("engine version"),
                target: Identifier::new(TARGET).expect("target id"),
                artifact_digest: Digest::from_bytes(TARGET.as_bytes()),
            },
        )
        .expect("execution receipt");
    let actual = GoldenDigests {
        run_id: receipt.run_id,
        output_digest: receipt.output_digest,
        telemetry_summary_digest: receipt.telemetry_summary_digest,
    };
    let expected: GoldenDigests =
        serde_json::from_str(EXPECTED_DIGESTS).expect("published digest vectors");

    assert_eq!(
        actual,
        expected,
        "Racing deterministic digests changed. Compare the canonical output and telemetry summary artifacts before updating this vector.\nActual:\n{}",
        serde_json::to_string_pretty(&actual).expect("actual digests must serialize")
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v1_digests_reject_semantic_mutations() {
    let output = run_golden_race();
    let evidence = RacingRunEvidenceV1::from_race_output(&output).expect("Racing evidence");
    let contract: DeterministicRunContractV1 =
        serde_json::from_str(CONTRACT).expect("Racing deterministic contract");

    let original_run_id = contract.run_id().expect("run id");
    let mut changed_contract = contract;
    changed_contract.random.seed = Seed::new(8);
    assert_ne!(
        changed_contract.run_id().expect("changed run id"),
        original_run_id,
        "an input contract mutation must change run_id"
    );

    let original_output_digest = evidence.output_digest().expect("output digest");
    let mut changed_output = evidence.output.clone();
    changed_output.total_time_ms += 1;
    assert_ne!(
        canonical_json_digest(&changed_output).expect("changed output digest"),
        original_output_digest,
        "a domain output mutation must change output_digest"
    );

    let original_summary_digest = evidence
        .telemetry_summary_digest()
        .expect("telemetry summary digest");
    let mut changed_summary =
        serde_json::to_value(evidence.telemetry_summary).expect("summary value");
    changed_summary["dropped_frame_count"] = serde_json::json!(1);
    assert_ne!(
        canonical_json_digest(&changed_summary).expect("changed summary digest"),
        original_summary_digest,
        "a telemetry summary mutation must change telemetry_summary_digest"
    );
}

fn run_golden_race() -> RaceOutput {
    let response = run_race_json(INPUT.to_string());
    serde_json::from_str(&response)
        .unwrap_or_else(|error| panic!("golden run returned invalid output: {error}: {response}"))
}

fn assert_artifact_eq<T>(label: &str, actual: &T, expected_json: &str)
where
    T: Debug + DeserializeOwned + PartialEq + Serialize,
{
    let expected: T = serde_json::from_str(expected_json)
        .unwrap_or_else(|error| panic!("invalid expected {label} artifact: {error}"));
    assert_eq!(
        actual,
        &expected,
        "{label} changed before digest comparison.\nActual:\n{}",
        serde_json::to_string_pretty(actual).expect("actual artifact must serialize")
    );
    assert_eq!(
        canonical_json_bytes(actual).expect("actual canonical artifact"),
        canonical_json_bytes(&expected).expect("expected canonical artifact"),
        "{label} canonical bytes changed"
    );
}

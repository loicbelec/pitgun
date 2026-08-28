use pitgun_contract::{
    Digest, IncrementalExecutionStreamBatchV1, IncrementalExecutionStreamCursorV1,
    IncrementalExecutionStreamDescriptorV1, canonical_json_digest,
};
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const FIXTURE_JSON: &str = include_str!("fixtures/incremental_execution_stream_v1.json");
const FIXTURE_DIGEST: &str =
    "sha256:91e1c3fcaa0216d2a16613faec80f44a961554895dade24940cf6dff0b7ff1d8";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgressFixture {
    phase: String,
    sample_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletionFixture {
    result_digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamFixture {
    descriptor: IncrementalExecutionStreamDescriptorV1,
    batches: Vec<IncrementalExecutionStreamBatchV1<ProgressFixture, CompletionFixture>>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn canonical_fixture_round_trips_and_completes() {
    let fixture: StreamFixture = serde_json::from_str(FIXTURE_JSON).expect("valid stream fixture");
    let expected_value: serde_json::Value =
        serde_json::from_str(FIXTURE_JSON).expect("fixture JSON value");
    let mut cursor = IncrementalExecutionStreamCursorV1::new();

    for batch in &fixture.batches {
        cursor.validate_next(batch).expect("ordered stream batch");
    }

    assert_eq!(
        fixture.descriptor.execution_id().to_string(),
        "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
    );
    assert_eq!(
        fixture.descriptor.model().id.as_str(),
        "pitgun.fixture-model"
    );
    assert_eq!(fixture.descriptor.clock().tick_numerator_us(), 50_000);
    assert_eq!(cursor.last_logical_tick(), Some(8));
    assert!(cursor.is_completed());
    assert_eq!(serde_json::to_value(&fixture).unwrap(), expected_value);
    assert_eq!(
        canonical_json_digest(&fixture)
            .expect("fixture digest")
            .to_string(),
        FIXTURE_DIGEST
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn fixture_rejects_sequence_and_completion_mutations() {
    let value: serde_json::Value = serde_json::from_str(FIXTURE_JSON).unwrap();
    let mut gap = value.clone();
    gap["batches"][1]["records"][0]["sequence"] = serde_json::json!(3);
    let mut invalid_execution_id = value.clone();
    invalid_execution_id["descriptor"]["execution_id"] =
        serde_json::json!("550e8400-e29b-41d4-a716-446655440000");
    let mut invalid_clock = value.clone();
    invalid_clock["descriptor"]["clock"]["tick_denominator"] = serde_json::json!(0);
    let mut record_after_completion = value;
    record_after_completion["batches"][1]["records"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "sequence": 3,
            "logical_tick": 9,
            "event": {
                "kind": "progress",
                "payload": {"phase": "late", "sample_count": 65}
            }
        }));

    let gap: StreamFixture = serde_json::from_value(gap).expect("each batch is locally valid");
    let mut cursor = IncrementalExecutionStreamCursorV1::new();
    cursor.validate_next(&gap.batches[0]).unwrap();
    assert!(cursor.validate_next(&gap.batches[1]).is_err());
    assert!(serde_json::from_value::<StreamFixture>(invalid_execution_id).is_err());
    assert!(serde_json::from_value::<StreamFixture>(invalid_clock).is_err());
    assert!(serde_json::from_value::<StreamFixture>(record_after_completion).is_err());
}

use pitgun_codec_json::{EventBatchDto, SESSION_ENVELOPE_SCHEMA_VERSION};
use pitgun_contract::game::v1::GameSimulationRequestV1;
use pitgun_source_physics::game::batching::telemetry_to_event_batches;
use pitgun_source_physics::game::simulate_request;
use serde::Serialize;
use wasm_bindgen::prelude::*;

const DEFAULT_SESSION_ID: &str = "wasm-session";
const DEFAULT_BATCH_EVENTS: usize = 260;

#[derive(Serialize)]
struct SessionEnvelopeOut {
    schema_version: u32,
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sent_at_ms: Option<i64>,
    batch: EventBatchDto,
}

#[wasm_bindgen]
pub fn simulate_batches(request_json: &str) -> String {
    match simulate_batches_inner(request_json) {
        Ok(payload) => payload,
        Err(err) => serde_json::json!({"error": err}).to_string(),
    }
}

fn simulate_batches_inner(request_json: &str) -> Result<String, String> {
    let request: GameSimulationRequestV1 =
        serde_json::from_str(request_json).map_err(|err| format!("invalid request JSON: {err}"))?;
    let result = simulate_request(&request).map_err(|err| format!("simulation failed: {err}"))?;
    let telemetry = result.telemetry.unwrap_or_default();

    let summary = telemetry_to_event_batches(&telemetry, DEFAULT_BATCH_EVENTS);
    let envelopes: Vec<SessionEnvelopeOut> = summary
        .batches
        .iter()
        .map(|batch| SessionEnvelopeOut {
            schema_version: SESSION_ENVELOPE_SCHEMA_VERSION,
            session_id: DEFAULT_SESSION_ID.to_string(),
            sent_at_ms: None,
            batch: EventBatchDto::from(batch),
        })
        .collect();

    serde_json::to_string(&envelopes).map_err(|err| format!("failed to serialize batches: {err}"))
}

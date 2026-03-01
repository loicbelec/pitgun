use std::collections::HashMap;

use pitgun_contract::TelemetryFrame;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub const EVENT_TYPE_SESSION_START: &str = "session.start";
pub const EVENT_TYPE_TELEMETRY_SAMPLE_BATCH: &str = "telemetry.sample_batch";
pub const EVENT_TYPE_SESSION_END: &str = "session.end";
pub const EVENT_TYPE_PURCHASE_ORDER_COMPLETED: &str = "purchase.order_completed";
pub const EVENT_TYPE_PITWALL_SESSION_CONFIGURED: &str = "pitwall.session_configured";

#[derive(Clone, Debug)]
pub struct EventEnvelope {
    pub schema_version: String,
    pub event_id: Uuid,
    pub ts: OffsetDateTime,
    pub player_id: String,
    pub session_id: String,
    pub event_type: String,
    pub payload: EventPayload,
}

impl EventEnvelope {
    pub fn payload_json(&self) -> Result<Value, serde_json::Error> {
        match &self.payload {
            EventPayload::SessionStart(payload) => serde_json::to_value(payload),
            EventPayload::TelemetrySampleBatch(payload) => serde_json::to_value(payload),
            EventPayload::SessionEnd(payload) => serde_json::to_value(payload),
            EventPayload::PurchaseOrderCompleted(payload) => serde_json::to_value(payload),
            EventPayload::PitWallSessionConfigured(payload) => serde_json::to_value(payload),
        }
    }
}

#[derive(Clone, Debug)]
pub enum EventPayload {
    SessionStart(SessionStartPayload),
    TelemetrySampleBatch(TelemetrySampleBatchPayload),
    SessionEnd(SessionEndPayload),
    PurchaseOrderCompleted(PurchaseOrderCompletedPayload),
    PitWallSessionConfigured(PitWallSessionConfiguredPayload),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionStartPayload {
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub track_id: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetrySampleBatchPayload {
    pub frames: Vec<TelemetryFrame>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionEndPayload {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub laps_completed: Option<u16>,
    #[serde(default)]
    pub best_lap_ms: Option<u64>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurchaseOrderCompletedPayload {
    pub order_id: String,
    pub currency: String,
    pub subtotal: f64,
    pub total: f64,
    #[serde(default)]
    pub tax: Option<f64>,
    #[serde(default)]
    pub discount: Option<f64>,
    pub line_items: Vec<PurchaseOrderLineItem>,
    #[serde(default)]
    pub purchased_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurchaseOrderLineItem {
    pub upgrade_id: String,
    pub quantity: u32,
    pub unit_price: f64,
    pub line_total: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PitWallSessionConfiguredPayload {
    pub run_id: String,
    pub track_id: String,
    pub vehicle_id: String,
    pub session_type: String,
    pub seed: u64,
    pub sampling_hz: f64,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub wasm_source_commit: Option<String>,
    #[serde(default)]
    pub wasm_build_time: Option<String>,
    pub setup: Value,
    pub setup_offsets: Value,
    pub effective_setup: Value,
    #[serde(default)]
    pub stint_strategy: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventEnvelopeWire {
    schema_version: String,
    event_id: Uuid,
    ts: String,
    player_id: String,
    session_id: String,
    event_type: String,
    payload: Value,
}

#[derive(Debug, Error)]
pub enum EnvelopeValidationError {
    #[error("invalid JSON payload: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("schema_version must be {expected} (got {actual})")]
    UnsupportedSchema { expected: String, actual: String },
    #[error("ts must be an ISO8601/RFC3339 timestamp")]
    InvalidTimestamp,
    #[error("player_id must be a non-empty string")]
    InvalidPlayerId,
    #[error("session_id must be a non-empty string")]
    InvalidSessionId,
    #[error("unsupported event_type: {0}")]
    UnsupportedEventType(String),
    #[error("invalid payload for event_type {event_type}: {message}")]
    InvalidPayload { event_type: String, message: String },
}

pub fn parse_event_envelope(
    raw: &str,
    expected_schema_version: &str,
) -> Result<EventEnvelope, EnvelopeValidationError> {
    let wire: EventEnvelopeWire = serde_json::from_str(raw)?;

    if wire.schema_version != expected_schema_version {
        return Err(EnvelopeValidationError::UnsupportedSchema {
            expected: expected_schema_version.to_string(),
            actual: wire.schema_version,
        });
    }

    let ts = OffsetDateTime::parse(&wire.ts, &Rfc3339)
        .map_err(|_| EnvelopeValidationError::InvalidTimestamp)?;

    let player_id = wire.player_id.trim().to_string();
    if player_id.is_empty() {
        return Err(EnvelopeValidationError::InvalidPlayerId);
    }

    let session_id = wire.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err(EnvelopeValidationError::InvalidSessionId);
    }

    let payload = parse_payload(&wire.event_type, wire.payload)?;

    Ok(EventEnvelope {
        schema_version: expected_schema_version.to_string(),
        event_id: wire.event_id,
        ts,
        player_id,
        session_id,
        event_type: wire.event_type,
        payload,
    })
}

fn parse_payload(
    event_type: &str,
    payload: Value,
) -> Result<EventPayload, EnvelopeValidationError> {
    match event_type {
        EVENT_TYPE_SESSION_START => {
            let payload: SessionStartPayload = serde_json::from_value(payload).map_err(|err| {
                EnvelopeValidationError::InvalidPayload {
                    event_type: event_type.to_string(),
                    message: err.to_string(),
                }
            })?;
            Ok(EventPayload::SessionStart(payload))
        }
        EVENT_TYPE_TELEMETRY_SAMPLE_BATCH => {
            let payload: TelemetrySampleBatchPayload =
                serde_json::from_value(payload).map_err(|err| {
                    EnvelopeValidationError::InvalidPayload {
                        event_type: event_type.to_string(),
                        message: err.to_string(),
                    }
                })?;

            if payload.frames.is_empty() {
                return Err(EnvelopeValidationError::InvalidPayload {
                    event_type: event_type.to_string(),
                    message: "frames must contain at least one TelemetryFrame".to_string(),
                });
            }

            Ok(EventPayload::TelemetrySampleBatch(payload))
        }
        EVENT_TYPE_SESSION_END => {
            let payload: SessionEndPayload = serde_json::from_value(payload).map_err(|err| {
                EnvelopeValidationError::InvalidPayload {
                    event_type: event_type.to_string(),
                    message: err.to_string(),
                }
            })?;
            Ok(EventPayload::SessionEnd(payload))
        }
        EVENT_TYPE_PURCHASE_ORDER_COMPLETED => {
            let payload: PurchaseOrderCompletedPayload =
                serde_json::from_value(payload).map_err(|err| {
                    EnvelopeValidationError::InvalidPayload {
                        event_type: event_type.to_string(),
                        message: err.to_string(),
                    }
                })?;

            validate_purchase_payload(event_type, &payload)?;
            Ok(EventPayload::PurchaseOrderCompleted(payload))
        }
        EVENT_TYPE_PITWALL_SESSION_CONFIGURED => {
            let payload: PitWallSessionConfiguredPayload =
                serde_json::from_value(payload).map_err(|err| {
                    EnvelopeValidationError::InvalidPayload {
                        event_type: event_type.to_string(),
                        message: err.to_string(),
                    }
                })?;

            validate_pitwall_session_payload(event_type, &payload)?;
            Ok(EventPayload::PitWallSessionConfigured(payload))
        }
        other => Err(EnvelopeValidationError::UnsupportedEventType(
            other.to_string(),
        )),
    }
}

fn validate_purchase_payload(
    event_type: &str,
    payload: &PurchaseOrderCompletedPayload,
) -> Result<(), EnvelopeValidationError> {
    if payload.order_id.trim().is_empty() {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "order_id must be a non-empty string".to_string(),
        });
    }

    let currency = payload.currency.trim();
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "currency must be a 3-letter ISO style code".to_string(),
        });
    }

    if !payload.subtotal.is_finite() || payload.subtotal < 0.0 {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "subtotal must be a finite number >= 0".to_string(),
        });
    }

    if !payload.total.is_finite() || payload.total < 0.0 {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "total must be a finite number >= 0".to_string(),
        });
    }

    if payload.line_items.is_empty() {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "line_items must contain at least one entry".to_string(),
        });
    }

    for line in &payload.line_items {
        if line.upgrade_id.trim().is_empty() {
            return Err(EnvelopeValidationError::InvalidPayload {
                event_type: event_type.to_string(),
                message: "line_items[].upgrade_id must be non-empty".to_string(),
            });
        }

        if line.quantity == 0 {
            return Err(EnvelopeValidationError::InvalidPayload {
                event_type: event_type.to_string(),
                message: "line_items[].quantity must be > 0".to_string(),
            });
        }

        if !line.unit_price.is_finite() || line.unit_price < 0.0 {
            return Err(EnvelopeValidationError::InvalidPayload {
                event_type: event_type.to_string(),
                message: "line_items[].unit_price must be a finite number >= 0".to_string(),
            });
        }

        if !line.line_total.is_finite() || line.line_total < 0.0 {
            return Err(EnvelopeValidationError::InvalidPayload {
                event_type: event_type.to_string(),
                message: "line_items[].line_total must be a finite number >= 0".to_string(),
            });
        }
    }

    if let Some(purchased_at) = &payload.purchased_at {
        OffsetDateTime::parse(purchased_at, &Rfc3339).map_err(|_| {
            EnvelopeValidationError::InvalidPayload {
                event_type: event_type.to_string(),
                message: "purchased_at must be ISO8601/RFC3339 when provided".to_string(),
            }
        })?;
    }

    Ok(())
}

fn validate_pitwall_session_payload(
    event_type: &str,
    payload: &PitWallSessionConfiguredPayload,
) -> Result<(), EnvelopeValidationError> {
    if payload.run_id.trim().is_empty() {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "run_id must be a non-empty string".to_string(),
        });
    }

    if payload.track_id.trim().is_empty() {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "track_id must be a non-empty string".to_string(),
        });
    }

    if payload.vehicle_id.trim().is_empty() {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "vehicle_id must be a non-empty string".to_string(),
        });
    }

    if payload.session_type.trim().is_empty() {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "session_type must be a non-empty string".to_string(),
        });
    }

    if !payload.sampling_hz.is_finite() || payload.sampling_hz <= 0.0 {
        return Err(EnvelopeValidationError::InvalidPayload {
            event_type: event_type.to_string(),
            message: "sampling_hz must be a finite number > 0".to_string(),
        });
    }

    if let Some(wasm_build_time) = &payload.wasm_build_time {
        OffsetDateTime::parse(wasm_build_time, &Rfc3339).map_err(|_| {
            EnvelopeValidationError::InvalidPayload {
                event_type: event_type.to_string(),
                message: "wasm_build_time must be ISO8601/RFC3339 when provided".to_string(),
            }
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        EVENT_TYPE_PITWALL_SESSION_CONFIGURED, EVENT_TYPE_TELEMETRY_SAMPLE_BATCH,
        parse_event_envelope,
    };

    #[test]
    fn parses_telemetry_sample_batch() {
        let payload = format!(
            r#"{{
                "schema_version": "pitgun-envelope-v1",
                "event_id": "9a593a28-22f3-48c8-bafe-d3076aad89ad",
                "ts": "2026-02-11T09:00:00Z",
                "player_id": "player-1",
                "session_id": "session-abc",
                "event_type": "{EVENT_TYPE_TELEMETRY_SAMPLE_BATCH}",
                "payload": {{
                    "frames": [{{
                        "session_id": 42,
                        "sequence": 1,
                        "timestamp_us": 1710000000000000,
                        "received_at_us": 1710000000000100,
                        "source_id": "sim-loop",
                        "samples": [],
                        "events": [],
                        "metadata": {{}}
                    }}]
                }}
            }}"#
        );

        let envelope =
            parse_event_envelope(&payload, "pitgun-envelope-v1").expect("payload should parse");

        assert_eq!(envelope.player_id, "player-1");
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let payload = r#"{
            "schema_version": "wrong",
            "event_id": "9a593a28-22f3-48c8-bafe-d3076aad89ad",
            "ts": "2026-02-11T09:00:00Z",
            "player_id": "player-1",
            "session_id": "session-abc",
            "event_type": "session.start",
            "payload": {}
        }"#;

        let err = parse_event_envelope(payload, "pitgun-envelope-v1").unwrap_err();
        assert!(
            err.to_string()
                .contains("schema_version must be pitgun-envelope-v1")
        );
    }

    #[test]
    fn parses_pitwall_session_configured() {
        let payload = format!(
            r#"{{
                "schema_version": "pitgun-envelope-v1",
                "event_id": "11111111-2222-4333-8444-555555555555",
                "ts": "2026-02-11T09:00:00Z",
                "player_id": "player-1",
                "session_id": "session-abc",
                "event_type": "{EVENT_TYPE_PITWALL_SESSION_CONFIGURED}",
                "payload": {{
                    "run_id": "run-1",
                    "track_id": "SPA",
                    "vehicle_id": "f1_2026",
                    "session_type": "FP1",
                    "seed": 1,
                    "sampling_hz": 5.0,
                    "game_version": "1.2",
                    "wasm_source_commit": "abc123",
                    "wasm_build_time": "2026-02-11T09:00:00Z",
                    "setup": {{
                        "aero": 8,
                        "chassis": 4,
                        "cooling": 2,
                        "engine": 3,
                        "downforce_slider": 0.5,
                        "gear_ratio_slider": 0.5
                    }},
                    "setup_offsets": {{
                        "aero": 2,
                        "chassis": 1,
                        "cooling": 0,
                        "engine": 0
                    }},
                    "effective_setup": {{
                        "aero": 10,
                        "chassis": 5,
                        "cooling": 2,
                        "engine": 3,
                        "downforce_slider": 0.5,
                        "gear_ratio_slider": 0.5
                    }},
                    "stint_strategy": {{
                        "stints": [
                            {{ "tire_id": "medium", "laps": 6 }},
                            {{ "tire_id": "hard", "laps": 4 }}
                        ],
                        "pit_laps": [6]
                    }}
                }}
            }}"#
        );

        let envelope =
            parse_event_envelope(&payload, "pitgun-envelope-v1").expect("payload should parse");

        assert_eq!(envelope.player_id, "player-1");
        assert_eq!(envelope.event_type, EVENT_TYPE_PITWALL_SESSION_CONFIGURED);
    }
}

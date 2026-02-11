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
        }
    }
}

#[derive(Clone, Debug)]
pub enum EventPayload {
    SessionStart(SessionStartPayload),
    TelemetrySampleBatch(TelemetrySampleBatchPayload),
    SessionEnd(SessionEndPayload),
    PurchaseOrderCompleted(PurchaseOrderCompletedPayload),
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

#[cfg(test)]
mod tests {
    use super::{EVENT_TYPE_TELEMETRY_SAMPLE_BATCH, parse_event_envelope};

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
}

use pitgun_core::{Event, EventBatch, SegmentAggregateRecord, SegmentTargetMetrics};
use serde::{Deserialize, Serialize};

pub const SESSION_ENVELOPE_SCHEMA_VERSION: u32 = 1;
pub const SESSION_ENVELOPE_JSON_V1_WIRE_ID: &str = "session-envelope-json-v1";

#[derive(Clone, Debug)]
pub struct SessionEnvelopeIn {
    pub schema_version: u32,
    pub session_id: String,
    pub sent_at_ms: Option<i64>,
    pub batch: EventBatch,
}

#[derive(Debug)]
pub enum SessionEnvelopeError {
    InvalidJson(serde_json::Error),
    UnsupportedSchema(u32),
    MissingSessionId,
}

impl std::fmt::Display for SessionEnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionEnvelopeError::InvalidJson(err) => write!(f, "invalid JSON payload: {err}"),
            SessionEnvelopeError::UnsupportedSchema(version) => {
                write!(
                    f,
                    "schema_version must be {} (got {version})",
                    SESSION_ENVELOPE_SCHEMA_VERSION
                )
            }
            SessionEnvelopeError::MissingSessionId => {
                write!(f, "session_id must be a non-empty string")
            }
        }
    }
}

impl std::error::Error for SessionEnvelopeError {}

impl From<serde_json::Error> for SessionEnvelopeError {
    fn from(value: serde_json::Error) -> Self {
        SessionEnvelopeError::InvalidJson(value)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct SessionEnvelopeDto {
    pub schema_version: u32,
    pub session_id: String,
    #[serde(default)]
    pub sent_at_ms: Option<i64>,
    pub batch: EventBatchDto,
}

pub fn deserialize_session_envelope(
    bytes: &[u8],
) -> Result<SessionEnvelopeIn, SessionEnvelopeError> {
    let dto: SessionEnvelopeDto = serde_json::from_slice(bytes)?;
    SessionEnvelopeIn::try_from(dto)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EventBatchDto {
    #[serde(default)]
    pub events: Vec<EventDto>,
    #[serde(default)]
    pub aggregates: Vec<SegmentAggregateRecordDto>,
    #[serde(default)]
    pub end_of_stream: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventDto {
    pub channel: String,
    #[serde(deserialize_with = "deserialize_ts_ns")]
    pub ts_ns: u64,
    pub value: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SegmentAggregateRecordDto {
    pub segment_key_channel: String,
    pub segment_value: f64,
    #[serde(default)]
    pub start_ts_ns: Option<u64>,
    #[serde(default)]
    pub end_ts_ns: Option<u64>,
    #[serde(default)]
    pub targets: Vec<SegmentTargetMetricsDto>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SegmentTargetMetricsDto {
    pub channel: String,
    #[serde(default)]
    pub count: Option<u64>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub sum: Option<f64>,
    #[serde(default)]
    pub mean: Option<f64>,
    #[serde(default)]
    pub stddev: Option<f64>,
}

impl TryFrom<SessionEnvelopeDto> for SessionEnvelopeIn {
    type Error = SessionEnvelopeError;

    fn try_from(value: SessionEnvelopeDto) -> Result<Self, Self::Error> {
        if value.schema_version != SESSION_ENVELOPE_SCHEMA_VERSION {
            return Err(SessionEnvelopeError::UnsupportedSchema(
                value.schema_version,
            ));
        }

        let session_id = value.session_id.trim().to_string();
        if session_id.is_empty() {
            return Err(SessionEnvelopeError::MissingSessionId);
        }

        Ok(Self {
            schema_version: value.schema_version,
            session_id,
            sent_at_ms: value.sent_at_ms,
            batch: value.batch.into(),
        })
    }
}

impl From<EventBatchDto> for EventBatch {
    fn from(value: EventBatchDto) -> Self {
        EventBatch {
            events: value.events.into_iter().map(Event::from).collect(),
            aggregates: Vec::new(), // aggregates are ignored for ingestion
            end_of_stream: value.end_of_stream,
        }
    }
}

impl From<&EventBatch> for EventBatchDto {
    fn from(value: &EventBatch) -> Self {
        EventBatchDto {
            events: value.events.iter().map(EventDto::from).collect(),
            aggregates: Vec::new(),
            end_of_stream: value.end_of_stream,
        }
    }
}

impl From<EventDto> for Event {
    fn from(value: EventDto) -> Self {
        Event {
            channel: value.channel,
            ts_ns: value.ts_ns,
            value: value.value,
        }
    }
}

impl From<&Event> for EventDto {
    fn from(value: &Event) -> Self {
        EventDto {
            channel: value.channel.clone(),
            ts_ns: value.ts_ns,
            value: value.value,
        }
    }
}

impl From<SegmentAggregateRecordDto> for SegmentAggregateRecord {
    fn from(value: SegmentAggregateRecordDto) -> Self {
        SegmentAggregateRecord {
            segment_key_channel: value.segment_key_channel,
            segment_value: value.segment_value,
            start_ts_ns: value.start_ts_ns,
            end_ts_ns: value.end_ts_ns,
            targets: value
                .targets
                .into_iter()
                .map(SegmentTargetMetrics::from)
                .collect(),
        }
    }
}

impl From<&SegmentAggregateRecord> for SegmentAggregateRecordDto {
    fn from(value: &SegmentAggregateRecord) -> Self {
        SegmentAggregateRecordDto {
            segment_key_channel: value.segment_key_channel.clone(),
            segment_value: value.segment_value,
            start_ts_ns: value.start_ts_ns,
            end_ts_ns: value.end_ts_ns,
            targets: value
                .targets
                .iter()
                .map(SegmentTargetMetricsDto::from)
                .collect(),
        }
    }
}

impl From<SegmentTargetMetricsDto> for SegmentTargetMetrics {
    fn from(value: SegmentTargetMetricsDto) -> Self {
        SegmentTargetMetrics {
            channel: value.channel,
            count: value.count,
            min: value.min,
            max: value.max,
            sum: value.sum,
            mean: value.mean,
            stddev: value.stddev,
        }
    }
}

impl From<&SegmentTargetMetrics> for SegmentTargetMetricsDto {
    fn from(value: &SegmentTargetMetrics) -> Self {
        SegmentTargetMetricsDto {
            channel: value.channel.clone(),
            count: value.count,
            min: value.min,
            max: value.max,
            sum: value.sum,
            mean: value.mean,
            stddev: value.stddev,
        }
    }
}

pub fn deserialize_ts_ns<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TsValue {
        Number(u64),
        String(String),
    }

    match TsValue::deserialize(deserializer)? {
        TsValue::Number(value) => Ok(value),
        TsValue::String(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|_| serde::de::Error::custom("ts_ns must be a number or numeric string")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ts_ns_as_string() {
        let payload = br#"{
            "schema_version": 1,
            "session_id": "abc-123",
            "batch": {
                "events": [
                    { "channel": "demo", "ts_ns": "1710000000000000000", "value": 1.23 }
                ]
            }
        }"#;

        let envelope = deserialize_session_envelope(payload).expect("payload should parse");
        assert_eq!(envelope.batch.events.len(), 1);
        assert_eq!(envelope.batch.events[0].ts_ns, 1_710_000_000_000_000_000);
        assert!(!envelope.batch.end_of_stream);
    }

    #[test]
    fn parses_ts_ns_as_number() {
        let payload = br#"{
            "schema_version": 1,
            "session_id": "abc-123",
            "batch": {
                "events": [
                    { "channel": "demo", "ts_ns": 1710000000000000000, "value": 2.0 }
                ],
                "end_of_stream": true
            }
        }"#;

        let envelope = deserialize_session_envelope(payload).expect("payload should parse");
        assert_eq!(envelope.batch.events[0].ts_ns, 1_710_000_000_000_000_000);
        assert!(envelope.batch.end_of_stream);
    }

    #[test]
    fn defaults_end_of_stream_to_false() {
        let payload = br#"{
            "schema_version": 1,
            "session_id": "abc-123",
            "batch": {
                "events": [
                    { "channel": "demo", "ts_ns": "1710000000000000000", "value": 3.14 }
                ]
            }
        }"#;

        let envelope = deserialize_session_envelope(payload).expect("payload should parse");
        assert!(!envelope.batch.end_of_stream);
    }
}

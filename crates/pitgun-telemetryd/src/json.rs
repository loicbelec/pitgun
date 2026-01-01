use pitgun_core::{Event, EventBatch, SegmentAggregateRecord, SegmentTargetMetrics};
use serde::{Deserialize, Serialize};

pub fn deserialize_event_batch(bytes: &[u8]) -> Result<EventBatch, serde_json::Error> {
    let dto: EventBatchDto = serde_json::from_slice(bytes)?;
    Ok(dto.into())
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

impl From<EventBatchDto> for EventBatch {
    fn from(value: EventBatchDto) -> Self {
        EventBatch {
            events: value.events.into_iter().map(Event::from).collect(),
            aggregates: value
                .aggregates
                .into_iter()
                .map(SegmentAggregateRecord::from)
                .collect(),
            end_of_stream: value.end_of_stream,
        }
    }
}

impl From<&EventBatch> for EventBatchDto {
    fn from(value: &EventBatch) -> Self {
        EventBatchDto {
            events: value.events.iter().map(EventDto::from).collect(),
            aggregates: value
                .aggregates
                .iter()
                .map(SegmentAggregateRecordDto::from)
                .collect(),
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

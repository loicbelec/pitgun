use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Event {
    pub channel: String,
    pub ts_ns: u64,
    pub value: f64,
}

#[derive(Clone, Debug, Default)]
pub struct EventBatch {
    pub events: Vec<Event>,
    pub aggregates: Vec<SegmentAggregateRecord>,
    pub end_of_stream: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SegmentAggregateRecord {
    pub segment_key_channel: String,
    pub segment_value: f64,
    pub start_ts_ns: Option<u64>,
    pub end_ts_ns: Option<u64>,
    pub targets: Vec<SegmentTargetMetrics>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SegmentTargetMetrics {
    pub channel: String,
    pub count: Option<u64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub sum: Option<f64>,
    pub mean: Option<f64>,
    pub stddev: Option<f64>,
}

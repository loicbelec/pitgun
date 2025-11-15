#[derive(Clone, Debug)]
pub struct Event {
    pub channel: String,
    pub ts_ns: u64,
    pub value: f64,
}

#[derive(Clone, Debug, Default)]
pub struct EventBatch {
    pub events: Vec<Event>,
}

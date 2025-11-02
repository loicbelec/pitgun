use bitflags::bitflags;

#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub run_id: String,
    pub car_id: String,
    pub track:  String,
    pub season: u32,
    pub rda_filtered: bool,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Quality: u32 {
        const RAW = 0b0001;
        const FILTERED = 0b0010;
        const INTERPOLATED = 0b0100;
    }
}

#[derive(Clone, Debug)]
pub struct Telemetry {
    pub channel: String,
    pub ts_ns:   u128,
    pub value:   f64,
    pub quality: Quality,
}

#[derive(Clone, Debug)]
pub enum Event {
    Telemetry(Telemetry),
    Heartbeat { ts_ns: u128 },
}

#[derive(Clone, Debug)]
pub struct EventBatch {
    pub meta:   SessionMeta,
    pub events: Vec<Event>,
}
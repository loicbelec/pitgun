use std::sync::Arc;
use chrono::{DateTime, Utc};
use arrow_array::ArrayRef;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub sensor_id: String,
    pub unit: Option<Unit>,
    pub tags: Vec<(String, String)>,  // ex: ("system","PU"), ("bank","A")
}

#[derive(Clone, Debug)]
pub struct Signal {
    pub name: String,
    pub timestamps: Arc<ArrayRef>,   // Arrow Timestamp(ns)
    pub values: Arc<ArrayRef>,       // Arrow Float64/Int64…
    pub meta: Meta,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub signals: Vec<Signal>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Unit {
    Celsius,
    Kpa,
    Rpm,
    Mps,
    Volt,
    Amp,
}
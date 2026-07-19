//! Telemetry Frame model for canonical data representation.
//!
//! This module defines the [`TelemetryFrame`] structure that serves as the
//! canonical data format for all telemetry sources. Every source produces
//! `TelemetryFrame` instances regardless of its underlying protocol.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │  UDP Source │     │  WS Source  │     │ Kafka Source│
//! └──────┬──────┘     └──────┬──────┘     └──────┬──────┘
//!        │                   │                   │
//!        └───────────────────┼───────────────────┘
//!                            ▼
//!                   ┌─────────────────┐
//!                   │ TelemetryFrame  │  ← Canonical format
//!                   └────────┬────────┘
//!                            │
//!                   ┌────────┴────────┐
//!                   │    Pipeline     │
//!                   └─────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust
//! use pitgun_contract::frame::{TelemetryFrame, Sample, SampleValue, SignalQuality};
//!
//! let frame = TelemetryFrame::builder()
//!     .session_id(12345)
//!     .timestamp_us(1700000000_000_000)
//!     .source_id("udp-f1-2024")
//!     .sample(Sample::new(1, SampleValue::U16(8500), SignalQuality::Good))
//!     .sample(Sample::new(2, SampleValue::F32(0.85), SignalQuality::Good))
//!     .build();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Unique identifier for a telemetry session.
pub type SessionId = u64;

/// Unique identifier for a parameter.
pub type ParameterId = u16;

/// Unique identifier for an event type.
pub type EventId = u16;

/// A canonical telemetry frame produced by any source.
///
/// This is the universal data structure that all telemetry sources produce.
/// It contains timing information, samples (parameter values), and events.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelemetryFrame {
    /// Unique identifier for the telemetry session.
    pub session_id: SessionId,
    /// Frame sequence number within the session (monotonically increasing).
    pub sequence: u64,
    /// Timestamp when the frame was captured (microseconds since Unix epoch).
    pub timestamp_us: i64,
    /// Timestamp when the frame was received by Pitgun (microseconds since Unix epoch).
    pub received_at_us: i64,
    /// Identifier of the source that produced this frame.
    pub source_id: String,
    /// Collection of parameter samples in this frame.
    pub samples: Vec<Sample>,
    /// Collection of events in this frame.
    pub events: Vec<Event>,
    /// Optional domain cycle index.
    #[serde(rename = "lap_number")]
    pub cycle_index: Option<u16>,
    /// Optional domain segment index.
    #[serde(rename = "sector")]
    pub segment_index: Option<u8>,
    /// Optional progress along the modeled path, in meters.
    #[serde(rename = "lap_distance_m")]
    pub progress_m: Option<f32>,
    /// Custom metadata as key-value pairs.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl TelemetryFrame {
    /// Creates a new frame builder.
    pub fn builder() -> TelemetryFrameBuilder {
        TelemetryFrameBuilder::new()
    }

    /// Creates a minimal frame with required fields.
    pub fn new(session_id: SessionId, timestamp_us: i64, source_id: impl Into<String>) -> Self {
        Self {
            session_id,
            sequence: 0,
            timestamp_us,
            received_at_us: now_us(),
            source_id: source_id.into(),
            samples: Vec::new(),
            events: Vec::new(),
            cycle_index: None,
            segment_index: None,
            progress_m: None,
            metadata: HashMap::new(),
        }
    }

    /// Returns the number of samples in this frame.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Returns the number of events in this frame.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Returns true if this frame has no samples and no events.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty() && self.events.is_empty()
    }

    /// Finds a sample by parameter ID.
    pub fn get_sample(&self, parameter_id: ParameterId) -> Option<&Sample> {
        self.samples.iter().find(|s| s.parameter_id == parameter_id)
    }

    /// Returns all samples with good signal quality.
    pub fn good_samples(&self) -> impl Iterator<Item = &Sample> {
        self.samples
            .iter()
            .filter(|s| s.quality == SignalQuality::Good)
    }

    /// Calculates the age of this frame (time since capture).
    pub fn age(&self) -> Duration {
        let now = now_us();
        if now > self.timestamp_us {
            Duration::from_micros((now - self.timestamp_us) as u64)
        } else {
            Duration::ZERO
        }
    }

    /// Calculates processing latency (received - captured).
    pub fn latency(&self) -> Duration {
        if self.received_at_us > self.timestamp_us {
            Duration::from_micros((self.received_at_us - self.timestamp_us) as u64)
        } else {
            Duration::ZERO
        }
    }
}

/// Builder for constructing [`TelemetryFrame`] instances.
#[derive(Clone, Debug, Default)]
pub struct TelemetryFrameBuilder {
    session_id: Option<SessionId>,
    sequence: u64,
    timestamp_us: Option<i64>,
    received_at_us: Option<i64>,
    source_id: Option<String>,
    samples: Vec<Sample>,
    events: Vec<Event>,
    cycle_index: Option<u16>,
    segment_index: Option<u8>,
    progress_m: Option<f32>,
    metadata: HashMap<String, String>,
}

impl TelemetryFrameBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the session ID.
    pub fn session_id(mut self, id: SessionId) -> Self {
        self.session_id = Some(id);
        self
    }

    /// Sets the frame sequence number.
    pub fn sequence(mut self, seq: u64) -> Self {
        self.sequence = seq;
        self
    }

    /// Sets the capture timestamp in microseconds.
    pub fn timestamp_us(mut self, ts: i64) -> Self {
        self.timestamp_us = Some(ts);
        self
    }

    /// Sets the received timestamp in microseconds.
    pub fn received_at_us(mut self, ts: i64) -> Self {
        self.received_at_us = Some(ts);
        self
    }

    /// Sets the source identifier.
    pub fn source_id(mut self, id: impl Into<String>) -> Self {
        self.source_id = Some(id.into());
        self
    }

    /// Adds a sample to the frame.
    pub fn sample(mut self, sample: Sample) -> Self {
        self.samples.push(sample);
        self
    }

    /// Adds multiple samples to the frame.
    pub fn samples(mut self, samples: impl IntoIterator<Item = Sample>) -> Self {
        self.samples.extend(samples);
        self
    }

    /// Adds an event to the frame.
    pub fn event(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }

    /// Adds multiple events to the frame.
    pub fn events(mut self, events: impl IntoIterator<Item = Event>) -> Self {
        self.events.extend(events);
        self
    }

    /// Sets the domain cycle index.
    pub fn cycle_index(mut self, cycle: u16) -> Self {
        self.cycle_index = Some(cycle);
        self
    }

    /// Sets the domain segment index.
    pub fn segment_index(mut self, segment: u8) -> Self {
        self.segment_index = Some(segment);
        self
    }

    /// Sets progress along the modeled path, in meters.
    pub fn progress_m(mut self, progress: f32) -> Self {
        self.progress_m = Some(progress);
        self
    }

    /// Adds a metadata key-value pair.
    pub fn meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Builds the TelemetryFrame.
    ///
    /// # Panics
    ///
    /// Panics if required fields (session_id, timestamp_us, source_id) are not set.
    pub fn build(self) -> TelemetryFrame {
        TelemetryFrame {
            session_id: self.session_id.expect("session_id is required"),
            sequence: self.sequence,
            timestamp_us: self.timestamp_us.expect("timestamp_us is required"),
            received_at_us: self.received_at_us.unwrap_or_else(now_us),
            source_id: self.source_id.expect("source_id is required"),
            samples: self.samples,
            events: self.events,
            cycle_index: self.cycle_index,
            segment_index: self.segment_index,
            progress_m: self.progress_m,
            metadata: self.metadata,
        }
    }

    /// Attempts to build the TelemetryFrame, returning None if required fields are missing.
    pub fn try_build(self) -> Option<TelemetryFrame> {
        Some(TelemetryFrame {
            session_id: self.session_id?,
            sequence: self.sequence,
            timestamp_us: self.timestamp_us?,
            received_at_us: self.received_at_us.unwrap_or_else(now_us),
            source_id: self.source_id?,
            samples: self.samples,
            events: self.events,
            cycle_index: self.cycle_index,
            segment_index: self.segment_index,
            progress_m: self.progress_m,
            metadata: self.metadata,
        })
    }
}

/// A single parameter sample within a frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    /// Unique identifier of the parameter.
    pub parameter_id: ParameterId,
    /// Raw value from the source.
    pub value: SampleValue,
    /// Signal quality indicator.
    pub quality: SignalQuality,
    /// Optional timestamp offset from frame timestamp (microseconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_offset_us: Option<i32>,
}

impl Sample {
    /// Creates a new sample.
    pub fn new(parameter_id: ParameterId, value: SampleValue, quality: SignalQuality) -> Self {
        Self {
            parameter_id,
            value,
            quality,
            timestamp_offset_us: None,
        }
    }

    /// Creates a new sample with good quality.
    pub fn good(parameter_id: ParameterId, value: SampleValue) -> Self {
        Self::new(parameter_id, value, SignalQuality::Good)
    }

    /// Creates a sample with a timestamp offset.
    pub fn with_offset(mut self, offset_us: i32) -> Self {
        self.timestamp_offset_us = Some(offset_us);
        self
    }

    /// Returns the value as f64 if possible.
    pub fn as_f64(&self) -> Option<f64> {
        self.value.as_f64()
    }

    /// Returns true if the signal quality is good.
    pub fn is_good(&self) -> bool {
        self.quality == SignalQuality::Good
    }
}

/// Raw value types supported in telemetry samples.
///
/// This enum covers all common data types found in telemetry protocols.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SampleValue {
    /// Boolean value.
    Bool(bool),
    /// Unsigned 8-bit integer.
    U8(u8),
    /// Unsigned 16-bit integer.
    U16(u16),
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Signed 8-bit integer.
    I8(i8),
    /// Signed 16-bit integer.
    I16(i16),
    /// Signed 32-bit integer.
    I32(i32),
    /// Signed 64-bit integer.
    I64(i64),
    /// 32-bit floating point.
    F32(f32),
    /// 64-bit floating point.
    F64(f64),
    /// Raw bytes (for complex or opaque data).
    Bytes(Vec<u8>),
    /// String value.
    String(String),
}

impl SampleValue {
    /// Converts the value to f64 if possible.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::U8(v) => Some(*v as f64),
            Self::U16(v) => Some(*v as f64),
            Self::U32(v) => Some(*v as f64),
            Self::U64(v) => Some(*v as f64),
            Self::I8(v) => Some(*v as f64),
            Self::I16(v) => Some(*v as f64),
            Self::I32(v) => Some(*v as f64),
            Self::I64(v) => Some(*v as f64),
            Self::F32(v) => Some(*v as f64),
            Self::F64(v) => Some(*v),
            Self::Bytes(_) => None,
            Self::String(s) => s.parse().ok(),
        }
    }

    /// Converts the value to i64 if possible.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Bool(b) => Some(if *b { 1 } else { 0 }),
            Self::U8(v) => Some(*v as i64),
            Self::U16(v) => Some(*v as i64),
            Self::U32(v) => Some(*v as i64),
            Self::U64(v) => i64::try_from(*v).ok(),
            Self::I8(v) => Some(*v as i64),
            Self::I16(v) => Some(*v as i64),
            Self::I32(v) => Some(*v as i64),
            Self::I64(v) => Some(*v),
            Self::F32(v) => Some(*v as i64),
            Self::F64(v) => Some(*v as i64),
            Self::Bytes(_) => None,
            Self::String(s) => s.parse().ok(),
        }
    }

    /// Returns the type name as a string.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::U8(_) => "u8",
            Self::U16(_) => "u16",
            Self::U32(_) => "u32",
            Self::U64(_) => "u64",
            Self::I8(_) => "i8",
            Self::I16(_) => "i16",
            Self::I32(_) => "i32",
            Self::I64(_) => "i64",
            Self::F32(_) => "f32",
            Self::F64(_) => "f64",
            Self::Bytes(_) => "bytes",
            Self::String(_) => "string",
        }
    }

    /// Returns the size in bytes of this value.
    pub fn size_bytes(&self) -> usize {
        match self {
            Self::Bool(_) => 1,
            Self::U8(_) | Self::I8(_) => 1,
            Self::U16(_) | Self::I16(_) => 2,
            Self::U32(_) | Self::I32(_) | Self::F32(_) => 4,
            Self::U64(_) | Self::I64(_) | Self::F64(_) => 8,
            Self::Bytes(b) => b.len(),
            Self::String(s) => s.len(),
        }
    }
}

/// Signal quality indicator for a sample.
///
/// Based on automotive/aerospace standards for signal quality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalQuality {
    /// Signal is valid and within expected parameters.
    Good,
    /// Signal is present but may be degraded (noise, intermittent).
    Degraded,
    /// Signal is invalid or out of range.
    Bad,
    /// No signal received (timeout, disconnection).
    NoSignal,
    /// Signal quality is unknown or not applicable.
    #[default]
    Unknown,
}

impl SignalQuality {
    /// Returns true if the signal is usable (Good or Degraded).
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Good | Self::Degraded)
    }

    /// Returns a numeric quality score (0-100).
    pub fn score(&self) -> u8 {
        match self {
            Self::Good => 100,
            Self::Degraded => 50,
            Self::Bad => 10,
            Self::NoSignal => 0,
            Self::Unknown => 0,
        }
    }
}

/// A discrete event within a telemetry frame.
///
/// Events represent instantaneous occurrences like gear shifts,
/// button presses, flags, or system alerts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier for the event type.
    pub event_id: EventId,
    /// Human-readable event name.
    pub name: String,
    /// Severity level of the event.
    pub severity: EventSeverity,
    /// Optional event data payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<EventData>,
    /// Optional timestamp offset from frame timestamp (microseconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_offset_us: Option<i32>,
}

impl Event {
    /// Creates a new event.
    pub fn new(event_id: EventId, name: impl Into<String>, severity: EventSeverity) -> Self {
        Self {
            event_id,
            name: name.into(),
            severity,
            data: None,
            timestamp_offset_us: None,
        }
    }

    /// Creates an info-level event.
    pub fn info(event_id: EventId, name: impl Into<String>) -> Self {
        Self::new(event_id, name, EventSeverity::Info)
    }

    /// Creates a warning-level event.
    pub fn warning(event_id: EventId, name: impl Into<String>) -> Self {
        Self::new(event_id, name, EventSeverity::Warning)
    }

    /// Creates an error-level event.
    pub fn error(event_id: EventId, name: impl Into<String>) -> Self {
        Self::new(event_id, name, EventSeverity::Error)
    }

    /// Creates a critical-level event.
    pub fn critical(event_id: EventId, name: impl Into<String>) -> Self {
        Self::new(event_id, name, EventSeverity::Critical)
    }

    /// Adds data to the event.
    pub fn with_data(mut self, data: EventData) -> Self {
        self.data = Some(data);
        self
    }

    /// Adds a timestamp offset.
    pub fn with_offset(mut self, offset_us: i32) -> Self {
        self.timestamp_offset_us = Some(offset_us);
        self
    }
}

/// Severity level for events.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
    /// Trace-level debug information.
    Trace,
    /// Debug information.
    Debug,
    /// Informational event (e.g., lap completed).
    #[default]
    Info,
    /// Warning condition (e.g., tire wear high).
    Warning,
    /// Error condition (e.g., engine overheating).
    Error,
    /// Critical condition requiring immediate attention.
    Critical,
}

impl EventSeverity {
    /// Returns true if this severity is at least Warning level.
    pub fn is_warning_or_higher(&self) -> bool {
        *self >= Self::Warning
    }

    /// Returns true if this severity is at least Error level.
    pub fn is_error_or_higher(&self) -> bool {
        *self >= Self::Error
    }
}

/// Optional data payload for an event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    /// Numeric value.
    Number(f64),
    /// String value.
    Text(String),
    /// Boolean value.
    Flag(bool),
    /// Key-value pairs.
    Map(HashMap<String, String>),
    /// Raw bytes.
    Bytes(Vec<u8>),
}

impl EventData {
    /// Creates a numeric event data.
    pub fn number(value: f64) -> Self {
        Self::Number(value)
    }

    /// Creates a text event data.
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// Creates a flag event data.
    pub fn flag(value: bool) -> Self {
        Self::Flag(value)
    }
}

/// Returns current time in microseconds since Unix epoch.
fn now_us() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_builder_basic() {
        let frame = TelemetryFrame::builder()
            .session_id(1)
            .timestamp_us(1000000)
            .source_id("test-source")
            .build();

        assert_eq!(frame.session_id, 1);
        assert_eq!(frame.timestamp_us, 1000000);
        assert_eq!(frame.source_id, "test-source");
        assert!(frame.samples.is_empty());
        assert!(frame.events.is_empty());
    }

    #[test]
    fn frame_builder_with_samples() {
        let frame = TelemetryFrame::builder()
            .session_id(1)
            .timestamp_us(1000000)
            .source_id("test")
            .sample(Sample::good(1, SampleValue::U16(8500)))
            .sample(Sample::good(2, SampleValue::F32(0.85)))
            .build();

        assert_eq!(frame.sample_count(), 2);
        assert!(frame.get_sample(1).is_some());
        assert!(frame.get_sample(99).is_none());
    }

    #[test]
    fn frame_builder_with_events() {
        let frame = TelemetryFrame::builder()
            .session_id(1)
            .timestamp_us(1000000)
            .source_id("test")
            .event(Event::info(1, "gear_shift"))
            .event(Event::warning(2, "tire_wear"))
            .build();

        assert_eq!(frame.event_count(), 2);
    }

    #[test]
    fn sample_value_conversions() {
        assert_eq!(SampleValue::U16(1000).as_f64(), Some(1000.0));
        let f32_value = SampleValue::F32(3.14)
            .as_f64()
            .expect("f32 should convert to f64");
        assert!((f32_value - 3.14).abs() < 1e-6);
        assert_eq!(SampleValue::Bool(true).as_f64(), Some(1.0));
        assert_eq!(SampleValue::Bool(false).as_f64(), Some(0.0));
        assert!(SampleValue::Bytes(vec![1, 2, 3]).as_f64().is_none());
        assert_eq!(SampleValue::String("42".to_string()).as_f64(), Some(42.0));
    }

    #[test]
    fn sample_value_type_names() {
        assert_eq!(SampleValue::U8(0).type_name(), "u8");
        assert_eq!(SampleValue::F64(0.0).type_name(), "f64");
        assert_eq!(SampleValue::Bytes(vec![]).type_name(), "bytes");
    }

    #[test]
    fn sample_value_sizes() {
        assert_eq!(SampleValue::Bool(true).size_bytes(), 1);
        assert_eq!(SampleValue::U16(0).size_bytes(), 2);
        assert_eq!(SampleValue::F32(0.0).size_bytes(), 4);
        assert_eq!(SampleValue::F64(0.0).size_bytes(), 8);
        assert_eq!(SampleValue::Bytes(vec![1, 2, 3]).size_bytes(), 3);
    }

    #[test]
    fn signal_quality_usable() {
        assert!(SignalQuality::Good.is_usable());
        assert!(SignalQuality::Degraded.is_usable());
        assert!(!SignalQuality::Bad.is_usable());
        assert!(!SignalQuality::NoSignal.is_usable());
    }

    #[test]
    fn signal_quality_scores() {
        assert_eq!(SignalQuality::Good.score(), 100);
        assert_eq!(SignalQuality::Degraded.score(), 50);
        assert_eq!(SignalQuality::Bad.score(), 10);
        assert_eq!(SignalQuality::NoSignal.score(), 0);
    }

    #[test]
    fn event_severity_ordering() {
        assert!(EventSeverity::Critical > EventSeverity::Error);
        assert!(EventSeverity::Error > EventSeverity::Warning);
        assert!(EventSeverity::Warning > EventSeverity::Info);
        assert!(EventSeverity::Info > EventSeverity::Debug);
    }

    #[test]
    fn event_creation() {
        let event = Event::warning(42, "low_fuel")
            .with_data(EventData::number(5.5))
            .with_offset(-1000);

        assert_eq!(event.event_id, 42);
        assert_eq!(event.name, "low_fuel");
        assert_eq!(event.severity, EventSeverity::Warning);
        assert!(event.data.is_some());
        assert_eq!(event.timestamp_offset_us, Some(-1000));
    }

    #[test]
    fn frame_good_samples() {
        let frame = TelemetryFrame::builder()
            .session_id(1)
            .timestamp_us(1000000)
            .source_id("test")
            .sample(Sample::good(1, SampleValue::U16(100)))
            .sample(Sample::new(2, SampleValue::U16(200), SignalQuality::Bad))
            .sample(Sample::good(3, SampleValue::U16(300)))
            .build();

        let good: Vec<_> = frame.good_samples().collect();
        assert_eq!(good.len(), 2);
    }

    #[test]
    fn frame_serde_roundtrip() {
        let frame = TelemetryFrame::builder()
            .session_id(1)
            .timestamp_us(1000000)
            .source_id("test")
            .sample(Sample::good(1, SampleValue::F32(3.14)))
            .event(Event::info(1, "test_event"))
            .cycle_index(5)
            .segment_index(2)
            .build();

        let json = serde_json::to_string(&frame).unwrap();
        let parsed: TelemetryFrame = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.session_id, frame.session_id);
        assert_eq!(parsed.sample_count(), 1);
        assert_eq!(parsed.event_count(), 1);
        assert_eq!(parsed.cycle_index, Some(5));
        assert_eq!(parsed.segment_index, Some(2));
    }

    #[test]
    fn try_build_returns_none_on_missing_fields() {
        let result = TelemetryFrameBuilder::new()
            .session_id(1)
            // missing timestamp_us and source_id
            .try_build();

        assert!(result.is_none());
    }
}

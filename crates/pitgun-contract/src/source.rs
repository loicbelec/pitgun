//! Telemetry Source abstractions for multi-source architecture.
//!
//! This module defines the core trait [`TelemetrySource`] that all telemetry
//! sources must implement, along with supporting types for source metadata,
//! statistics, and configuration.
//!
//! # Architecture
//!
//! The multi-source architecture is inspired by ECUBridge McLaren's approach:
//! - Each source implements a common interface (`TelemetrySource`)
//! - Sources produce canonical `TelemetryFrame` structs
//! - Multiple sources can run concurrently
//! - Sources are identified by type and metadata
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_contract::source::{TelemetrySource, SourceType, SourceConfig};
//!
//! struct MyUdpSource { /* ... */ }
//!
//! #[async_trait]
//! impl TelemetrySource for MyUdpSource {
//!     fn name(&self) -> &str { "my-udp-source" }
//!     fn source_type(&self) -> SourceType { SourceType::Udp }
//!     // ...
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::frame::TelemetryFrame;

/// Result type for source operations.
pub type SourceResult<T> = Result<T, SourceError>;

/// Errors that can occur during source operations.
#[derive(Clone, Debug)]
pub enum SourceError {
    /// Source is not connected or has been disconnected.
    NotConnected,
    /// Failed to connect to the data source.
    ConnectionFailed(String),
    /// Timeout waiting for data or connection.
    Timeout(Duration),
    /// Invalid configuration provided.
    InvalidConfig(String),
    /// The source channel is closed.
    ChannelClosed,
    /// Protocol-specific error.
    ProtocolError(String),
    /// I/O error occurred.
    IoError(String),
    /// Source is already running.
    AlreadyRunning,
    /// Source is not running.
    NotRunning,
    /// Generic internal error.
    Internal(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => write!(f, "source not connected"),
            Self::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            Self::Timeout(d) => write!(f, "timeout after {:?}", d),
            Self::InvalidConfig(msg) => write!(f, "invalid configuration: {msg}"),
            Self::ChannelClosed => write!(f, "channel closed"),
            Self::ProtocolError(msg) => write!(f, "protocol error: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::AlreadyRunning => write!(f, "source is already running"),
            Self::NotRunning => write!(f, "source is not running"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for SourceError {}

impl From<std::io::Error> for SourceError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e.to_string())
    }
}

/// Type of telemetry data source.
///
/// Each source type may have different characteristics:
/// - **UDP**: Low-latency, connectionless, may lose packets
/// - **WebSocket**: Bidirectional, connection-oriented, web-friendly
/// - **Kafka**: High-throughput, persistent, distributed
/// - **MQTT**: Lightweight pub/sub, IoT-friendly
/// - **Physics**: Internal simulation physics engine
/// - **CAN**: CAN bus hardware interface
/// - **File**: Replay from recorded file (CSV, binary, etc.)
/// - **Custom**: User-defined source type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// UDP socket source (e.g., F1 game telemetry)
    Udp,
    /// WebSocket connection
    WebSocket,
    /// Apache Kafka consumer
    Kafka,
    /// MQTT subscriber
    Mqtt,
    /// Internal physics simulation
    Physics,
    /// CAN bus interface
    Can,
    /// File-based replay source
    File,
    /// HTTP/REST polling source
    Http,
    /// gRPC streaming source
    Grpc,
    /// Custom/user-defined source
    Custom,
}

impl SourceType {
    /// Returns a human-readable name for the source type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::WebSocket => "websocket",
            Self::Kafka => "kafka",
            Self::Mqtt => "mqtt",
            Self::Physics => "physics",
            Self::Can => "can",
            Self::File => "file",
            Self::Http => "http",
            Self::Grpc => "grpc",
            Self::Custom => "custom",
        }
    }

    /// Returns true if this source type is network-based.
    pub fn is_network(&self) -> bool {
        matches!(
            self,
            Self::Udp | Self::WebSocket | Self::Kafka | Self::Mqtt | Self::Http | Self::Grpc
        )
    }

    /// Returns true if this source type is connection-oriented.
    pub fn is_connection_oriented(&self) -> bool {
        matches!(
            self,
            Self::WebSocket | Self::Kafka | Self::Mqtt | Self::Grpc
        )
    }
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Current state of a telemetry source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    /// Source is created but not yet started.
    Idle,
    /// Source is attempting to connect.
    Connecting,
    /// Source is connected and receiving data.
    Running,
    /// Source is temporarily paused.
    Paused,
    /// Source is reconnecting after a disconnection.
    Reconnecting,
    /// Source has stopped (gracefully or due to error).
    Stopped,
    /// Source encountered a fatal error.
    Error,
}

impl SourceState {
    /// Returns true if the source is actively processing data.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Reconnecting)
    }

    /// Returns true if the source can be started.
    pub fn can_start(&self) -> bool {
        matches!(self, Self::Idle | Self::Stopped | Self::Error)
    }
}

/// Runtime statistics for a telemetry source.
///
/// These metrics are useful for monitoring source health,
/// debugging performance issues, and capacity planning.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceStats {
    /// Total number of frames received since start.
    pub frames_received: u64,
    /// Total number of bytes received since start.
    pub bytes_received: u64,
    /// Number of frames dropped (buffer overflow, etc.).
    pub frames_dropped: u64,
    /// Number of decode/parse errors.
    pub decode_errors: u64,
    /// Number of connection errors.
    pub connection_errors: u64,
    /// Current frames per second rate.
    pub current_fps: f64,
    /// Average frames per second since start.
    pub average_fps: f64,
    /// Peak frames per second observed.
    pub peak_fps: f64,
    /// Average latency from source to frame emission (microseconds).
    pub avg_latency_us: u64,
    /// 99th percentile latency (microseconds).
    pub p99_latency_us: u64,
    /// Time of last received frame (Unix timestamp microseconds).
    pub last_frame_at_us: Option<i64>,
    /// Time when source was started (Unix timestamp microseconds).
    pub started_at_us: Option<i64>,
    /// Total uptime in seconds.
    pub uptime_secs: f64,
    /// Number of reconnection attempts.
    pub reconnect_count: u32,
}

impl SourceStats {
    /// Creates a new empty stats instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets all statistics to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Records a successfully received frame.
    pub fn record_frame(&mut self, bytes: usize) {
        self.frames_received += 1;
        self.bytes_received += bytes as u64;
    }

    /// Records a dropped frame.
    pub fn record_drop(&mut self) {
        self.frames_dropped += 1;
    }

    /// Records a decode error.
    pub fn record_decode_error(&mut self) {
        self.decode_errors += 1;
    }

    /// Returns the drop rate as a percentage.
    pub fn drop_rate(&self) -> f64 {
        let total = self.frames_received + self.frames_dropped;
        if total == 0 {
            0.0
        } else {
            (self.frames_dropped as f64 / total as f64) * 100.0
        }
    }

    /// Returns the error rate as a percentage.
    pub fn error_rate(&self) -> f64 {
        if self.frames_received == 0 {
            0.0
        } else {
            (self.decode_errors as f64 / self.frames_received as f64) * 100.0
        }
    }
}

/// Metadata describing a telemetry source.
///
/// This information is attached to each `TelemetryFrame` to identify
/// its origin and provide context for processing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceMetadata {
    /// Unique identifier for this source instance.
    pub source_id: String,
    /// Human-readable name of the source.
    pub name: String,
    /// Type of the source.
    pub source_type: SourceType,
    /// Version of the source implementation.
    pub version: String,
    /// Protocol version being used (if applicable).
    pub protocol_version: Option<String>,
    /// Connection endpoint (e.g., "udp://0.0.0.0:20777", "ws://server:8080").
    pub endpoint: Option<String>,
    /// Additional tags for categorization.
    pub tags: Vec<String>,
    /// Priority level for frame merging (higher = more priority).
    pub priority: u8,
    /// Whether this source provides authoritative timestamps.
    pub authoritative_time: bool,
}

impl SourceMetadata {
    /// Creates a new metadata instance with required fields.
    pub fn new(
        source_id: impl Into<String>,
        name: impl Into<String>,
        source_type: SourceType,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            name: name.into(),
            source_type,
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: None,
            endpoint: None,
            tags: Vec::new(),
            priority: 50,
            authoritative_time: false,
        }
    }

    /// Builder method to set the protocol version.
    pub fn with_protocol_version(mut self, version: impl Into<String>) -> Self {
        self.protocol_version = Some(version.into());
        self
    }

    /// Builder method to set the endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Builder method to add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Builder method to set priority.
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Builder method to mark as authoritative time source.
    pub fn with_authoritative_time(mut self, authoritative: bool) -> Self {
        self.authoritative_time = authoritative;
        self
    }
}

/// Configuration for source behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Whether to automatically reconnect on disconnection.
    pub auto_reconnect: bool,
    /// Maximum number of reconnection attempts (0 = unlimited).
    pub max_reconnect_attempts: u32,
    /// Delay between reconnection attempts.
    pub reconnect_delay: Duration,
    /// Maximum backoff delay for exponential backoff.
    pub max_reconnect_delay: Duration,
    /// Buffer size for the output channel.
    pub channel_buffer_size: usize,
    /// Timeout for connection attempts.
    pub connect_timeout: Duration,
    /// Timeout for read operations.
    pub read_timeout: Option<Duration>,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            max_reconnect_attempts: 10,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            channel_buffer_size: 1024,
            connect_timeout: Duration::from_secs(10),
            read_timeout: None,
        }
    }
}

/// Universal interface for telemetry data sources.
///
/// All telemetry sources (UDP, WebSocket, Kafka, MQTT, Physics, CAN, etc.)
/// must implement this trait to be usable in the Pitgun pipeline.
///
/// # Lifecycle
///
/// 1. Create the source with source-specific configuration
/// 2. Call `start()` with a channel sender to begin receiving frames
/// 3. Frames are sent to the channel as they arrive
/// 4. Call `stop()` to gracefully shut down the source
///
/// # Thread Safety
///
/// Sources must be `Send + Sync` to allow concurrent access from
/// multiple async tasks.
///
/// # Example
///
/// ```rust,ignore
/// let (tx, mut rx) = mpsc::unbounded_channel();
/// let mut source = MyUdpSource::new(config)?;
///
/// source.start(tx).await?;
///
/// while let Some(frame) = rx.recv().await {
///     process_frame(frame);
/// }
///
/// source.stop().await?;
/// ```
#[async_trait]
pub trait TelemetrySource: Send + Sync {
    /// Returns the human-readable name of this source.
    fn name(&self) -> &str;

    /// Returns the unique identifier for this source instance.
    fn source_id(&self) -> &str;

    /// Returns the type of this source.
    fn source_type(&self) -> SourceType;

    /// Returns the current state of the source.
    fn state(&self) -> SourceState;

    /// Returns metadata describing this source.
    fn metadata(&self) -> SourceMetadata;

    /// Returns current statistics for this source.
    fn stats(&self) -> SourceStats;

    /// Starts the source and begins sending frames to the provided channel.
    ///
    /// The source will continuously read data, decode it into `TelemetryFrame`
    /// instances, and send them to the provided channel.
    ///
    /// # Arguments
    ///
    /// * `tx` - Unbounded channel sender for emitting telemetry frames.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is already running or cannot connect.
    async fn start(&mut self, tx: mpsc::UnboundedSender<TelemetryFrame>) -> SourceResult<()>;

    /// Stops the source gracefully.
    ///
    /// This will close any connections and stop sending frames.
    /// The source can be restarted by calling `start()` again.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is not running.
    async fn stop(&mut self) -> SourceResult<()>;

    /// Pauses the source temporarily.
    ///
    /// The source remains connected but stops emitting frames.
    /// Call `resume()` to continue receiving data.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is not running.
    async fn pause(&mut self) -> SourceResult<()> {
        Err(SourceError::Internal("pause not supported".into()))
    }

    /// Resumes a paused source.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is not paused.
    async fn resume(&mut self) -> SourceResult<()> {
        Err(SourceError::Internal("resume not supported".into()))
    }

    /// Checks if the source is healthy and receiving data.
    ///
    /// Returns `true` if the source is connected and has received
    /// data within a reasonable time window.
    fn is_healthy(&self) -> bool {
        self.state() == SourceState::Running
    }

    /// Returns the configuration for this source.
    fn config(&self) -> &SourceConfig;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_type_display() {
        assert_eq!(SourceType::Udp.to_string(), "udp");
        assert_eq!(SourceType::WebSocket.to_string(), "websocket");
        assert_eq!(SourceType::Kafka.to_string(), "kafka");
        assert_eq!(SourceType::Custom.to_string(), "custom");
    }

    #[test]
    fn source_type_is_network() {
        assert!(SourceType::Udp.is_network());
        assert!(SourceType::WebSocket.is_network());
        assert!(SourceType::Kafka.is_network());
        assert!(!SourceType::Physics.is_network());
        assert!(!SourceType::File.is_network());
        assert!(!SourceType::Can.is_network());
    }

    #[test]
    fn source_type_is_connection_oriented() {
        assert!(!SourceType::Udp.is_connection_oriented());
        assert!(SourceType::WebSocket.is_connection_oriented());
        assert!(SourceType::Kafka.is_connection_oriented());
        assert!(SourceType::Mqtt.is_connection_oriented());
        assert!(!SourceType::File.is_connection_oriented());
    }

    #[test]
    fn source_state_can_start() {
        assert!(SourceState::Idle.can_start());
        assert!(SourceState::Stopped.can_start());
        assert!(SourceState::Error.can_start());
        assert!(!SourceState::Running.can_start());
        assert!(!SourceState::Connecting.can_start());
    }

    #[test]
    fn source_stats_drop_rate() {
        let mut stats = SourceStats::new();
        assert_eq!(stats.drop_rate(), 0.0);

        stats.frames_received = 90;
        stats.frames_dropped = 10;
        assert!((stats.drop_rate() - 10.0).abs() < 0.01);
    }

    #[test]
    fn source_stats_record_frame() {
        let mut stats = SourceStats::new();
        stats.record_frame(1024);
        stats.record_frame(512);

        assert_eq!(stats.frames_received, 2);
        assert_eq!(stats.bytes_received, 1536);
    }

    #[test]
    fn source_metadata_builder() {
        let meta = SourceMetadata::new("src-001", "F1 UDP", SourceType::Udp)
            .with_protocol_version("2024")
            .with_endpoint("udp://0.0.0.0:20777")
            .with_tag("f1")
            .with_tag("telemetry")
            .with_priority(100)
            .with_authoritative_time(true);

        assert_eq!(meta.source_id, "src-001");
        assert_eq!(meta.name, "F1 UDP");
        assert_eq!(meta.source_type, SourceType::Udp);
        assert_eq!(meta.protocol_version, Some("2024".to_string()));
        assert_eq!(meta.endpoint, Some("udp://0.0.0.0:20777".to_string()));
        assert_eq!(meta.tags.len(), 2);
        assert_eq!(meta.priority, 100);
        assert!(meta.authoritative_time);
    }

    #[test]
    fn source_config_default() {
        let config = SourceConfig::default();
        assert!(config.auto_reconnect);
        assert_eq!(config.max_reconnect_attempts, 10);
        assert_eq!(config.channel_buffer_size, 1024);
    }

    #[test]
    fn source_error_display() {
        let err = SourceError::ConnectionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "connection failed: timeout");

        let err = SourceError::Timeout(Duration::from_secs(5));
        assert!(err.to_string().contains("5s"));
    }

    #[test]
    fn source_type_serde() {
        let json = serde_json::to_string(&SourceType::WebSocket).unwrap();
        assert_eq!(json, "\"web_socket\"");

        let parsed: SourceType = serde_json::from_str("\"kafka\"").unwrap();
        assert_eq!(parsed, SourceType::Kafka);
    }
}

//! Pitgun Kafka Telemetry Source
//!
//! This crate provides a Kafka consumer source that implements the
//! [`TelemetrySource`] trait from `pitgun-contract`.
//!
//! # Features
//!
//! - **Consumer groups**: Scalable consumption with Kafka consumer groups
//! - **Multi-topic**: Subscribe to multiple topics with pattern matching
//! - **JSON decoding**: Decode JSON-encoded telemetry messages
//! - **Offset management**: Configurable offset commit behavior
//! - **Stats tracking**: Messages consumed, decoded, lag metrics
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_source_kafka::{KafkaSource, KafkaSourceConfig};
//! use pitgun_contract::TelemetrySource;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = KafkaSourceConfig::new("localhost:9092")
//!         .with_group_id("pitgun-telemetry")
//!         .with_topics(vec!["telemetry.raw", "telemetry.processed"])
//!         .with_source_id("kafka-source");
//!
//!     let mut source = KafkaSource::new(config)?;
//!     source.start().await?;
//!
//!     let mut rx = source.subscribe();
//!     while let Some(frame) = rx.recv().await {
//!         println!("Frame: {} samples from partition {}", 
//!             frame.sample_count(), 
//!             frame.metadata().get("partition").unwrap_or(&"?".to_string()));
//!     }
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use futures::StreamExt;
use pitgun_contract::{
    Sample, SampleValue, SignalQuality, SourceConfig, SourceError, SourceMetadata, SourceResult,
    SourceState, SourceStats, SourceType, TelemetryFrame, TelemetryFrameBuilder, TelemetrySource,
};
use rdkafka::{
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    message::Message,
    TopicPartitionList,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

/// Configuration for the Kafka source
#[derive(Clone, Debug)]
pub struct KafkaSourceConfig {
    /// Kafka bootstrap servers (e.g., "localhost:9092")
    pub bootstrap_servers: String,
    /// Consumer group ID
    pub group_id: String,
    /// Topics to subscribe to
    pub topics: Vec<String>,
    /// Source ID
    pub source_id: String,
    /// Auto offset reset (earliest, latest)
    pub auto_offset_reset: String,
    /// Enable auto commit
    pub enable_auto_commit: bool,
    /// Session timeout in milliseconds
    pub session_timeout_ms: u32,
    /// Channel capacity for frame broadcasting
    pub channel_capacity: usize,
    /// Additional Kafka client configuration
    pub extra_config: HashMap<String, String>,
}

impl KafkaSourceConfig {
    /// Creates a new configuration with the given bootstrap servers
    pub fn new(bootstrap_servers: impl Into<String>) -> Self {
        Self {
            bootstrap_servers: bootstrap_servers.into(),
            group_id: "pitgun-consumer".into(),
            topics: vec!["telemetry".into()],
            source_id: "kafka-source".into(),
            auto_offset_reset: "latest".into(),
            enable_auto_commit: true,
            session_timeout_ms: 6000,
            channel_capacity: 1024,
            extra_config: HashMap::new(),
        }
    }

    /// Sets the consumer group ID
    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = group_id.into();
        self
    }

    /// Sets the topics to subscribe to
    pub fn with_topics(mut self, topics: Vec<impl Into<String>>) -> Self {
        self.topics = topics.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single topic to subscribe to
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    /// Sets the source ID
    pub fn with_source_id(mut self, id: impl Into<String>) -> Self {
        self.source_id = id.into();
        self
    }

    /// Sets the auto offset reset behavior
    pub fn with_auto_offset_reset(mut self, reset: impl Into<String>) -> Self {
        self.auto_offset_reset = reset.into();
        self
    }

    /// Enables or disables auto commit
    pub fn with_auto_commit(mut self, enable: bool) -> Self {
        self.enable_auto_commit = enable;
        self
    }

    /// Sets the session timeout
    pub fn with_session_timeout(mut self, timeout: Duration) -> Self {
        self.session_timeout_ms = timeout.as_millis() as u32;
        self
    }

    /// Sets the channel capacity
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Adds extra Kafka configuration
    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_config.insert(key.into(), value.into());
        self
    }

    /// Builds the rdkafka ClientConfig
    fn build_client_config(&self) -> ClientConfig {
        let mut config = ClientConfig::new();
        config
            .set("bootstrap.servers", &self.bootstrap_servers)
            .set("group.id", &self.group_id)
            .set("auto.offset.reset", &self.auto_offset_reset)
            .set("enable.auto.commit", self.enable_auto_commit.to_string())
            .set("session.timeout.ms", self.session_timeout_ms.to_string());

        for (key, value) in &self.extra_config {
            config.set(key, value);
        }

        config
    }
}

impl From<KafkaSourceConfig> for SourceConfig {
    fn from(_cfg: KafkaSourceConfig) -> Self {
        SourceConfig::default()
    }
}

/// Statistics for the Kafka source
#[derive(Debug, Default)]
struct KafkaStats {
    messages_received: AtomicU64,
    messages_decoded: AtomicU64,
    decode_errors: AtomicU64,
    bytes_received: AtomicU64,
    frames_produced: AtomicU64,
    samples_produced: AtomicU64,
    partitions_assigned: AtomicU64,
}

impl KafkaStats {
    fn to_source_stats(&self, start_time: Instant) -> SourceStats {
        let frames = self.frames_produced.load(Ordering::Relaxed);
        let elapsed = start_time.elapsed().as_secs_f64();
        let fps = if elapsed > 0.0 {
            frames as f64 / elapsed
        } else {
            0.0
        };

        SourceStats {
            frames_received: frames,
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            frames_dropped: 0,
            decode_errors: self.decode_errors.load(Ordering::Relaxed),
            connection_errors: 0,
            current_fps: fps,
            average_fps: fps,
            peak_fps: fps,
            avg_latency_us: 0,
            p99_latency_us: 0,
            last_frame_at_us: None,
            started_at_us: None,
            uptime_secs: elapsed,
            reconnect_count: 0,
        }
    }
}

/// Kafka telemetry source
pub struct KafkaSource {
    config: KafkaSourceConfig,
    source_config: SourceConfig,
    state: Arc<RwLock<SourceState>>,
    stats: Arc<KafkaStats>,
    start_time: Instant,
    frame_tx: Option<mpsc::UnboundedSender<TelemetryFrame>>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl KafkaSource {
    /// Creates a new Kafka source
    pub fn new(config: KafkaSourceConfig) -> SourceResult<Self> {
        let source_config = SourceConfig::default();
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Ok(Self {
            config,
            source_config,
            state: Arc::new(RwLock::new(SourceState::Idle)),
            stats: Arc::new(KafkaStats::default()),
            start_time: Instant::now(),
            frame_tx: None,
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
        })
    }

    /// Decode a JSON message into a TelemetryFrame
    fn decode_message(
        payload: &[u8],
        source_id: &str,
        topic: &str,
        partition: i32,
        offset: i64,
        sequence: &mut u64,
    ) -> Option<TelemetryFrame> {
        // Try to parse as JSON
        let json: serde_json::Value = serde_json::from_slice(payload).ok()?;

        let samples: Vec<Sample> = if let Some(obj) = json.as_object() {
            obj.iter()
                .enumerate()
                .filter_map(|(i, (_, v))| {
                    v.as_f64().map(|val| Sample {
                        parameter_id: i as u16,
                        value: SampleValue::F64(val),
                        quality: SignalQuality::Good,
                        timestamp_offset_us: None,
                    })
                })
                .collect()
        } else {
            return None;
        };

        if samples.is_empty() {
            return None;
        }

        *sequence += 1;

        // Extract timestamp from message if present
        let timestamp_us = json
            .get("timestamp")
            .or_else(|| json.get("ts"))
            .or_else(|| json.get("time"))
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_micros() as i64)
                    .unwrap_or(0)
            });

        let mut builder = TelemetryFrameBuilder::new()
            .source_id(source_id)
            .sequence(*sequence)
            .timestamp_us(timestamp_us)
            .samples(samples);

        // Add Kafka metadata
        builder = builder
            .meta("topic", topic)
            .meta("partition", &partition.to_string())
            .meta("offset", &offset.to_string());

        Some(builder.build())
    }

    /// Run the Kafka consumer loop
    async fn consumer_loop(
        config: KafkaSourceConfig,
        state: Arc<RwLock<SourceState>>,
        stats: Arc<KafkaStats>,
        frame_tx: mpsc::UnboundedSender<TelemetryFrame>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {

        // Create consumer
        let consumer: StreamConsumer = match config.build_client_config().create() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to create Kafka consumer: {}", e);
                *state.write().await = SourceState::Error;
                return;
            }
        };

        // Subscribe to topics
        let topics: Vec<&str> = config.topics.iter().map(String::as_str).collect();
        if let Err(e) = consumer.subscribe(&topics) {
            eprintln!("Failed to subscribe to topics: {}", e);
            *state.write().await = SourceState::Error;
            return;
        }

        *state.write().await = SourceState::Running;

        let mut sequence = 0u64;
        let mut message_stream = consumer.stream();

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                msg_result = message_stream.next() => {
                    match msg_result {
                        Some(Ok(msg)) => {
                            stats.messages_received.fetch_add(1, Ordering::Relaxed);

                            if let Some(payload) = msg.payload() {
                                stats.bytes_received.fetch_add(payload.len() as u64, Ordering::Relaxed);

                                let topic = msg.topic();
                                let partition = msg.partition();
                                let offset = msg.offset();

                                if let Some(frame) = Self::decode_message(
                                    payload,
                                    &config.source_id,
                                    topic,
                                    partition,
                                    offset,
                                    &mut sequence,
                                ) {
                                    stats.messages_decoded.fetch_add(1, Ordering::Relaxed);
                                    stats.frames_produced.fetch_add(1, Ordering::Relaxed);
                                    stats.samples_produced.fetch_add(frame.sample_count() as u64, Ordering::Relaxed);
                                    let _ = frame_tx.send(frame);
                                } else {
                                    stats.decode_errors.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        Some(Err(e)) => {
                            eprintln!("Kafka error: {}", e);
                            stats.decode_errors.fetch_add(1, Ordering::Relaxed);
                        }
                        None => {
                            // Stream ended
                            break;
                        }
                    }
                }
            }
        }

        *state.write().await = SourceState::Stopped;
    }
}

#[async_trait]
impl TelemetrySource for KafkaSource {
    fn name(&self) -> &str {
        &self.config.source_id
    }

    fn source_id(&self) -> &str {
        &self.config.source_id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Kafka
    }

    fn state(&self) -> SourceState {
        self.state.try_read().map(|s| *s).unwrap_or(SourceState::Idle)
    }

    fn metadata(&self) -> SourceMetadata {
        SourceMetadata::new(
            &self.config.source_id,
            format!("Kafka Source ({})", self.config.bootstrap_servers),
            SourceType::Kafka,
        )
        .with_endpoint(&self.config.bootstrap_servers)
        .with_tag("kafka")
        .with_tag(&self.config.group_id)
    }

    fn stats(&self) -> SourceStats {
        self.stats.to_source_stats(self.start_time)
    }

    fn config(&self) -> &SourceConfig {
        &self.source_config
    }

    async fn start(&mut self, tx: mpsc::UnboundedSender<TelemetryFrame>) -> SourceResult<()> {
        let current_state = self.state();
        if matches!(current_state, SourceState::Running) {
            return Err(SourceError::AlreadyRunning);
        }

        let shutdown_rx = self
            .shutdown_rx
            .take()
            .ok_or_else(|| SourceError::AlreadyRunning)?;

        *self.state.write().await = SourceState::Connecting;
        self.start_time = Instant::now();
        self.frame_tx = Some(tx.clone());

        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            Self::consumer_loop(config, state, stats, tx, shutdown_rx).await;
        });

        // Wait for running state
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if matches!(self.state(), SourceState::Running) {
                return Ok(());
            }
        }

        Err(SourceError::Timeout(Duration::from_secs(5)))
    }

    async fn stop(&mut self) -> SourceResult<()> {
        let _ = self.shutdown_tx.send(()).await;

        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if matches!(self.state(), SourceState::Stopped) {
                return Ok(());
            }
        }

        Err(SourceError::Timeout(Duration::from_millis(500)))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder() {
        let config = KafkaSourceConfig::new("localhost:9092")
            .with_group_id("test-group")
            .with_topics(vec!["topic1", "topic2"])
            .with_source_id("test-kafka")
            .with_auto_offset_reset("earliest");

        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.group_id, "test-group");
        assert_eq!(config.topics, vec!["topic1", "topic2"]);
        assert_eq!(config.source_id, "test-kafka");
        assert_eq!(config.auto_offset_reset, "earliest");
    }

    #[test]
    fn config_to_source_config() {
        let config = KafkaSourceConfig::new("localhost:9092")
            .with_source_id("my-kafka")
            .with_topics(vec!["telemetry"]);

        let source_config: SourceConfig = config.into();
        assert_eq!(source_config.source_id, "my-kafka");
        assert_eq!(
            source_config.options.get("bootstrap_servers"),
            Some(&"localhost:9092".to_string())
        );
    }

    #[test]
    fn decode_json_message() {
        let payload = br#"{"speed": 245.5, "rpm": 12500.0, "throttle": 0.85}"#;
        let mut seq = 0u64;

        let frame = KafkaSource::decode_message(
            payload,
            "test",
            "telemetry",
            0,
            100,
            &mut seq,
        );

        assert!(frame.is_some());
        assert_eq!(seq, 1);

        let frame = frame.unwrap();
        assert_eq!(frame.sample_count(), 3);
    }
}

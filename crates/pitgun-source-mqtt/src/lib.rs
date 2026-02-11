//! Pitgun MQTT Telemetry Source
//!
//! This crate provides an MQTT subscriber source that implements the
//! [`TelemetrySource`] trait from `pitgun-contract`.
//!
//! # Features
//!
//! - **Wildcard topics**: Subscribe using MQTT wildcards (+, #)
//! - **QoS configuration**: Support for QoS 0, 1, 2
//! - **JSON decoding**: Decode JSON-encoded telemetry messages
//! - **Auto-reconnect**: Automatic reconnection on disconnect
//! - **Stats tracking**: Messages received, decoded, connection status
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_source_mqtt::{MqttSource, MqttSourceConfig, QoS};
//! use pitgun_contract::TelemetrySource;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = MqttSourceConfig::new("localhost", 1883)
//!         .with_client_id("pitgun-subscriber")
//!         .with_topic("telemetry/#", QoS::AtLeastOnce)
//!         .with_source_id("mqtt-source");
//!
//!     let mut source = MqttSource::new(config)?;
//!     source.start().await?;
//!
//!     let mut rx = source.subscribe();
//!     while let Some(frame) = rx.recv().await {
//!         println!("Frame from topic: {:?}", frame.metadata().get("topic"));
//!     }
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use pitgun_contract::{
    Sample, SampleValue, SignalQuality, SourceConfig, SourceError, SourceMetadata, SourceResult,
    SourceState, SourceStats, SourceType, TelemetryFrame, TelemetryFrameBuilder, TelemetrySource,
};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};

/// MQTT Quality of Service levels
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QoS {
    /// At most once delivery (fire and forget)
    #[default]
    AtMostOnce,
    /// At least once delivery (acknowledged)
    AtLeastOnce,
    /// Exactly once delivery (guaranteed)
    ExactlyOnce,
}

impl From<QoS> for rumqttc::QoS {
    fn from(qos: QoS) -> Self {
        match qos {
            QoS::AtMostOnce => rumqttc::QoS::AtMostOnce,
            QoS::AtLeastOnce => rumqttc::QoS::AtLeastOnce,
            QoS::ExactlyOnce => rumqttc::QoS::ExactlyOnce,
        }
    }
}

impl QoS {
    /// Returns the numeric QoS level
    pub fn level(&self) -> u8 {
        match self {
            QoS::AtMostOnce => 0,
            QoS::AtLeastOnce => 1,
            QoS::ExactlyOnce => 2,
        }
    }
}

/// Topic subscription with QoS
#[derive(Clone, Debug)]
pub struct TopicSubscription {
    pub topic: String,
    pub qos: QoS,
}

impl TopicSubscription {
    pub fn new(topic: impl Into<String>, qos: QoS) -> Self {
        Self {
            topic: topic.into(),
            qos,
        }
    }
}

/// Configuration for the MQTT source
#[derive(Clone, Debug)]
pub struct MqttSourceConfig {
    /// MQTT broker host
    pub host: String,
    /// MQTT broker port
    pub port: u16,
    /// Client ID
    pub client_id: String,
    /// Topics to subscribe to with QoS
    pub subscriptions: Vec<TopicSubscription>,
    /// Source ID
    pub source_id: String,
    /// Keep alive interval
    pub keep_alive: Duration,
    /// Clean session flag
    pub clean_session: bool,
    /// Username for authentication
    pub username: Option<String>,
    /// Password for authentication
    pub password: Option<String>,
    /// Channel capacity for frame broadcasting
    pub channel_capacity: usize,
    /// Incoming message buffer capacity
    pub incoming_buffer_capacity: usize,
}

impl MqttSourceConfig {
    /// Creates a new configuration with the given host and port
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            client_id: format!("pitgun-{}", std::process::id()),
            subscriptions: vec![],
            source_id: "mqtt-source".into(),
            keep_alive: Duration::from_secs(30),
            clean_session: true,
            username: None,
            password: None,
            channel_capacity: 1024,
            incoming_buffer_capacity: 256,
        }
    }

    /// Sets the client ID
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = client_id.into();
        self
    }

    /// Adds a topic subscription
    pub fn with_topic(mut self, topic: impl Into<String>, qos: QoS) -> Self {
        self.subscriptions.push(TopicSubscription::new(topic, qos));
        self
    }

    /// Adds multiple topic subscriptions with the same QoS
    pub fn with_topics(mut self, topics: Vec<impl Into<String>>, qos: QoS) -> Self {
        for topic in topics {
            self.subscriptions.push(TopicSubscription::new(topic, qos));
        }
        self
    }

    /// Sets the source ID
    pub fn with_source_id(mut self, id: impl Into<String>) -> Self {
        self.source_id = id.into();
        self
    }

    /// Sets the keep alive interval
    pub fn with_keep_alive(mut self, duration: Duration) -> Self {
        self.keep_alive = duration;
        self
    }

    /// Sets clean session flag
    pub fn with_clean_session(mut self, clean: bool) -> Self {
        self.clean_session = clean;
        self
    }

    /// Sets authentication credentials
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Sets the channel capacity
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Builds the rumqttc MqttOptions
    fn build_mqtt_options(&self) -> MqttOptions {
        let mut options = MqttOptions::new(&self.client_id, &self.host, self.port);
        options.set_keep_alive(self.keep_alive);
        options.set_clean_session(self.clean_session);

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            options.set_credentials(username, password);
        }

        options
    }
}

impl From<MqttSourceConfig> for SourceConfig {
    fn from(cfg: MqttSourceConfig) -> Self {
        SourceConfig {
            channel_buffer_size: cfg.channel_capacity,
            ..SourceConfig::default()
        }
    }
}

/// Statistics for the MQTT source
#[derive(Debug, Default)]
struct MqttStats {
    messages_received: AtomicU64,
    messages_decoded: AtomicU64,
    decode_errors: AtomicU64,
    bytes_received: AtomicU64,
    frames_produced: AtomicU64,
    samples_produced: AtomicU64,
    reconnect_count: AtomicU64,
}

impl MqttStats {
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
            reconnect_count: self.reconnect_count.load(Ordering::Relaxed) as u32,
        }
    }
}

/// MQTT telemetry source
pub struct MqttSource {
    config: MqttSourceConfig,
    source_config: SourceConfig,
    state: Arc<RwLock<SourceState>>,
    stats: Arc<MqttStats>,
    start_time: Instant,
    frame_tx: Option<mpsc::UnboundedSender<TelemetryFrame>>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl MqttSource {
    /// Creates a new MQTT source
    pub fn new(config: MqttSourceConfig) -> SourceResult<Self> {
        if config.subscriptions.is_empty() {
            return Err(SourceError::InvalidConfig("no topics configured".into()));
        }

        let source_config: SourceConfig = config.clone().into();
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Ok(Self {
            config,
            source_config,
            state: Arc::new(RwLock::new(SourceState::Idle)),
            stats: Arc::new(MqttStats::default()),
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

        let frame = TelemetryFrameBuilder::new()
            .session_id(1)
            .source_id(source_id)
            .sequence(*sequence)
            .timestamp_us(timestamp_us)
            .samples(samples)
            .meta("topic", topic)
            .build();

        Some(frame)
    }

    /// Run the MQTT event loop
    async fn event_loop(
        config: MqttSourceConfig,
        state: Arc<RwLock<SourceState>>,
        stats: Arc<MqttStats>,
        frame_tx: mpsc::UnboundedSender<TelemetryFrame>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        let mqtt_options = config.build_mqtt_options();
        let (client, mut eventloop) =
            AsyncClient::new(mqtt_options, config.incoming_buffer_capacity);

        // Subscribe to all configured topics
        for sub in &config.subscriptions {
            if let Err(e) = client.subscribe(&sub.topic, sub.qos.into()).await {
                eprintln!("Failed to subscribe to {}: {}", sub.topic, e);
            }
        }

        *state.write().await = SourceState::Running;

        let mut sequence = 0u64;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    // Disconnect gracefully
                    let _ = client.disconnect().await;
                    break;
                }
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            stats.messages_received.fetch_add(1, Ordering::Relaxed);
                            stats.bytes_received.fetch_add(publish.payload.len() as u64, Ordering::Relaxed);

                            if let Some(frame) = Self::decode_message(
                                &publish.payload,
                                &config.source_id,
                                &publish.topic,
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
                        Ok(Event::Incoming(Packet::ConnAck(_))) => {
                            // Connected/reconnected
                            stats.reconnect_count.fetch_add(1, Ordering::Relaxed);
                        }
                        Ok(_) => {
                            // Other events (PingResp, SubAck, etc.)
                        }
                        Err(e) => {
                            eprintln!("MQTT error: {}", e);
                            stats.decode_errors.fetch_add(1, Ordering::Relaxed);
                            // rumqttc will auto-reconnect
                        }
                    }
                }
            }
        }

        *state.write().await = SourceState::Stopped;
    }
}

#[async_trait]
impl TelemetrySource for MqttSource {
    fn name(&self) -> &str {
        &self.config.source_id
    }

    fn source_id(&self) -> &str {
        &self.config.source_id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Mqtt
    }

    fn state(&self) -> SourceState {
        // Use try_read to avoid blocking, fallback to Idle
        self.state
            .try_read()
            .map(|s| *s)
            .unwrap_or(SourceState::Idle)
    }

    fn metadata(&self) -> SourceMetadata {
        SourceMetadata::new(
            &self.config.source_id,
            format!("MQTT Source ({}:{})", self.config.host, self.config.port),
            SourceType::Mqtt,
        )
        .with_endpoint(format!("mqtt://{}:{}", self.config.host, self.config.port))
        .with_tag("mqtt")
        .with_tag(&self.config.client_id)
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
            Self::event_loop(config, state, stats, tx, shutdown_rx).await;
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
        let config = MqttSourceConfig::new("localhost", 1883)
            .with_client_id("test-client")
            .with_topic("telemetry/#", QoS::AtLeastOnce)
            .with_topic("sensors/+/data", QoS::ExactlyOnce)
            .with_source_id("test-mqtt");

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 1883);
        assert_eq!(config.client_id, "test-client");
        assert_eq!(config.subscriptions.len(), 2);
        assert_eq!(config.subscriptions[0].topic, "telemetry/#");
        assert_eq!(config.subscriptions[1].qos, QoS::ExactlyOnce);
    }

    #[test]
    fn config_with_credentials() {
        let config =
            MqttSourceConfig::new("mqtt.example.com", 8883).with_credentials("user", "password");

        assert_eq!(config.username, Some("user".to_string()));
        assert_eq!(config.password, Some("password".to_string()));
    }

    #[test]
    fn qos_levels() {
        assert_eq!(QoS::AtMostOnce.level(), 0);
        assert_eq!(QoS::AtLeastOnce.level(), 1);
        assert_eq!(QoS::ExactlyOnce.level(), 2);
    }

    #[test]
    fn decode_json_message() {
        let payload = br#"{"speed": 245.5, "rpm": 12500.0}"#;
        let mut seq = 0u64;

        let frame = MqttSource::decode_message(payload, "test", "telemetry/car1", &mut seq);

        assert!(frame.is_some());
        assert_eq!(seq, 1);

        let frame = frame.unwrap();
        assert_eq!(frame.sample_count(), 2);
    }

    #[test]
    fn config_requires_topics() {
        let config = MqttSourceConfig::new("localhost", 1883);
        let result = MqttSource::new(config);
        assert!(result.is_err());
    }
}

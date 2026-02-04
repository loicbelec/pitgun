//! Async WebSocket Telemetry Source
//!
//! This module provides an async WebSocket telemetry source that implements the
//! [`TelemetrySource`] trait from `pitgun-contract`.
//!
//! # Features
//!
//! - **Async/await**: Non-blocking operation with tokio
//! - **Auto-reconnect**: Configurable reconnection with exponential backoff
//! - **JSON decoding**: Decode JSON-encoded telemetry frames
//! - **Stats tracking**: Messages received, decoded, errors
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_source_ws::{AsyncWsSource, WsSourceConfig};
//! use pitgun_contract::TelemetrySource;
//!
//! let config = WsSourceConfig::new("ws://localhost:8080/telemetry")
//!     .with_reconnect(true)
//!     .with_source_id("ws-source");
//!
//! let mut source = AsyncWsSource::new(config);
//! source.start().await?;
//!
//! let mut rx = source.subscribe();
//! while let Some(frame) = rx.recv().await {
//!     println!("Received frame: {} samples", frame.sample_count());
//! }
//! ```

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use pitgun_contract::{
    Sample, SampleValue, SignalQuality, SourceConfig, SourceError, SourceMetadata, SourceResult,
    SourceState, SourceStats, SourceType, TelemetryFrame, TelemetryFrameBuilder, TelemetrySource,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

/// Configuration for the async WebSocket source
#[derive(Clone, Debug)]
pub struct WsSourceConfig {
    /// WebSocket URL (e.g., "ws://localhost:8080/telemetry")
    pub url: String,
    /// Source ID
    pub source_id: String,
    /// Whether to auto-reconnect on disconnect
    pub reconnect: bool,
    /// Initial reconnect delay
    pub reconnect_delay: Duration,
    /// Maximum reconnect delay
    pub max_reconnect_delay: Duration,
    /// Channel capacity for frame broadcasting
    pub channel_capacity: usize,
}

impl WsSourceConfig {
    /// Creates a new configuration with the given WebSocket URL
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            source_id: "ws-source".into(),
            reconnect: true,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            channel_capacity: 1024,
        }
    }

    /// Sets the source ID
    pub fn with_source_id(mut self, id: impl Into<String>) -> Self {
        self.source_id = id.into();
        self
    }

    /// Enables or disables auto-reconnect
    pub fn with_reconnect(mut self, reconnect: bool) -> Self {
        self.reconnect = reconnect;
        self
    }

    /// Sets the initial reconnect delay
    pub fn with_reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay = delay;
        self
    }

    /// Sets the maximum reconnect delay
    pub fn with_max_reconnect_delay(mut self, delay: Duration) -> Self {
        self.max_reconnect_delay = delay;
        self
    }

    /// Sets the channel capacity
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }
}

impl From<WsSourceConfig> for SourceConfig {
    fn from(cfg: WsSourceConfig) -> Self {
        SourceConfig::new("websocket", &cfg.source_id)
            .with_option("url", cfg.url)
            .with_option("reconnect", cfg.reconnect.to_string())
            .with_option("reconnect_delay_ms", cfg.reconnect_delay.as_millis().to_string())
            .with_option("channel_capacity", cfg.channel_capacity.to_string())
    }
}

/// Statistics for the WebSocket source
#[derive(Debug, Default)]
struct WsStats {
    messages_received: AtomicU64,
    messages_decoded: AtomicU64,
    decode_errors: AtomicU64,
    bytes_received: AtomicU64,
    frames_produced: AtomicU64,
    samples_produced: AtomicU64,
    reconnect_count: AtomicU64,
}

impl WsStats {
    fn to_source_stats(&self, start_time: Instant) -> SourceStats {
        let samples = self.samples_produced.load(Ordering::Relaxed);
        let elapsed = start_time.elapsed().as_secs_f64();
        let rate = if elapsed > 0.0 {
            samples as f64 / elapsed
        } else {
            0.0
        };

        SourceStats {
            frames_produced: self.frames_produced.load(Ordering::Relaxed),
            samples_produced: samples,
            bytes_processed: self.bytes_received.load(Ordering::Relaxed),
            errors: self.decode_errors.load(Ordering::Relaxed),
            sample_rate_hz: rate,
            uptime: start_time.elapsed(),
            custom: [
                (
                    "messages_received".into(),
                    self.messages_received.load(Ordering::Relaxed) as f64,
                ),
                (
                    "messages_decoded".into(),
                    self.messages_decoded.load(Ordering::Relaxed) as f64,
                ),
                (
                    "reconnect_count".into(),
                    self.reconnect_count.load(Ordering::Relaxed) as f64,
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}

/// Async WebSocket telemetry source
pub struct AsyncWsSource {
    config: WsSourceConfig,
    state: Arc<RwLock<SourceState>>,
    stats: Arc<WsStats>,
    start_time: Instant,
    frame_tx: broadcast::Sender<TelemetryFrame>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl AsyncWsSource {
    /// Creates a new async WebSocket source
    pub fn new(config: WsSourceConfig) -> Self {
        let (frame_tx, _) = broadcast::channel(config.channel_capacity);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Self {
            config,
            state: Arc::new(RwLock::new(SourceState::Stopped)),
            stats: Arc::new(WsStats::default()),
            start_time: Instant::now(),
            frame_tx,
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
        }
    }

    /// Decode a JSON message into a TelemetryFrame
    fn decode_message(text: &str, source_id: &str, sequence: &mut u64) -> Option<TelemetryFrame> {
        // Try to parse as session envelope first
        if let Ok(envelope) = pitgun_codec_json::deserialize_session_envelope(text.as_bytes()) {
            let samples: Vec<Sample> = envelope
                .batch
                .events
                .iter()
                .enumerate()
                .map(|(i, e)| Sample {
                    parameter_id: i as u16,
                    value: SampleValue::F64(e.value),
                    quality: SignalQuality::Good,
                })
                .collect();

            if !samples.is_empty() {
                *sequence += 1;
                return Some(
                    TelemetryFrameBuilder::new()
                        .source_id(source_id)
                        .sequence(*sequence)
                        .samples(samples)
                        .build(),
                );
            }
        }

        // Try to parse as generic JSON telemetry
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(obj) = json.as_object() {
                let samples: Vec<Sample> = obj
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (_, v))| {
                        v.as_f64().map(|val| Sample {
                            parameter_id: i as u16,
                            value: SampleValue::F64(val),
                            quality: SignalQuality::Good,
                        })
                    })
                    .collect();

                if !samples.is_empty() {
                    *sequence += 1;
                    return Some(
                        TelemetryFrameBuilder::new()
                            .source_id(source_id)
                            .sequence(*sequence)
                            .samples(samples)
                            .build(),
                    );
                }
            }
        }

        None
    }

    /// Run the WebSocket receive loop with reconnection
    async fn receive_loop(
        config: WsSourceConfig,
        state: Arc<RwLock<SourceState>>,
        stats: Arc<WsStats>,
        frame_tx: broadcast::Sender<TelemetryFrame>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        let url = match Url::parse(&config.url) {
            Ok(u) => u,
            Err(e) => {
                *state.write().await = SourceState::Error(format!("invalid URL: {}", e));
                return;
            }
        };

        let mut reconnect_delay = config.reconnect_delay;
        let mut sequence = 0u64;

        loop {
            // Try to connect
            *state.write().await = SourceState::Starting;

            let ws_stream = match connect_async(&url).await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    if config.reconnect {
                        eprintln!("WebSocket connection failed: {}, retrying in {:?}", e, reconnect_delay);
                        stats.reconnect_count.fetch_add(1, Ordering::Relaxed);

                        tokio::select! {
                            _ = shutdown_rx.recv() => {
                                *state.write().await = SourceState::Stopped;
                                return;
                            }
                            _ = tokio::time::sleep(reconnect_delay) => {}
                        }

                        // Exponential backoff
                        reconnect_delay = std::cmp::min(reconnect_delay * 2, config.max_reconnect_delay);
                        continue;
                    } else {
                        *state.write().await = SourceState::Error(format!("connection failed: {}", e));
                        return;
                    }
                }
            };

            // Reset reconnect delay on successful connection
            reconnect_delay = config.reconnect_delay;
            *state.write().await = SourceState::Running;

            let (mut write, mut read) = ws_stream.split();

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        // Send close frame
                        let _ = write.send(Message::Close(None)).await;
                        *state.write().await = SourceState::Stopped;
                        return;
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                stats.messages_received.fetch_add(1, Ordering::Relaxed);
                                stats.bytes_received.fetch_add(text.len() as u64, Ordering::Relaxed);

                                if let Some(frame) = Self::decode_message(&text, &config.source_id, &mut sequence) {
                                    stats.messages_decoded.fetch_add(1, Ordering::Relaxed);
                                    stats.frames_produced.fetch_add(1, Ordering::Relaxed);
                                    stats.samples_produced.fetch_add(frame.sample_count() as u64, Ordering::Relaxed);
                                    let _ = frame_tx.send(frame);
                                } else {
                                    stats.decode_errors.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Some(Ok(Message::Binary(data))) => {
                                stats.messages_received.fetch_add(1, Ordering::Relaxed);
                                stats.bytes_received.fetch_add(data.len() as u64, Ordering::Relaxed);
                                // Binary messages not yet supported
                            }
                            Some(Ok(Message::Ping(payload))) => {
                                let _ = write.send(Message::Pong(payload)).await;
                            }
                            Some(Ok(Message::Pong(_))) => {}
                            Some(Ok(Message::Close(_))) => {
                                break; // Connection closed, try reconnect
                            }
                            Some(Ok(Message::Frame(_))) => {}
                            Some(Err(e)) => {
                                eprintln!("WebSocket error: {}", e);
                                stats.decode_errors.fetch_add(1, Ordering::Relaxed);
                                break; // Try reconnect
                            }
                            None => {
                                break; // Stream ended, try reconnect
                            }
                        }
                    }
                }
            }

            // Connection lost, try reconnect if enabled
            if !config.reconnect {
                *state.write().await = SourceState::Stopped;
                return;
            }

            stats.reconnect_count.fetch_add(1, Ordering::Relaxed);
            *state.write().await = SourceState::Starting;
        }
    }
}

#[async_trait]
impl TelemetrySource for AsyncWsSource {
    fn name(&self) -> &str {
        &self.config.source_id
    }

    fn source_type(&self) -> SourceType {
        SourceType::WebSocket
    }

    fn metadata(&self) -> SourceMetadata {
        SourceMetadata {
            id: self.config.source_id.clone(),
            name: format!("WebSocket Source ({})", self.config.url),
            source_type: SourceType::WebSocket,
            version: Some(env!("CARGO_PKG_VERSION").into()),
            description: Some(format!(
                "WebSocket telemetry source connected to {}",
                self.config.url
            )),
            capabilities: vec!["decode".into(), "reconnect".into()],
            tags: vec!["websocket".into(), "json".into()],
        }
    }

    async fn start(&mut self) -> SourceResult<()> {
        let current_state = *self.state.read().await;
        if matches!(current_state, SourceState::Running) {
            return Err(SourceError::invalid_state("already running"));
        }

        let shutdown_rx = self
            .shutdown_rx
            .take()
            .ok_or_else(|| SourceError::invalid_state("source already started"))?;

        *self.state.write().await = SourceState::Starting;
        self.start_time = Instant::now();

        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let stats = Arc::clone(&self.stats);
        let frame_tx = self.frame_tx.clone();

        tokio::spawn(async move {
            Self::receive_loop(config, state, stats, frame_tx, shutdown_rx).await;
        });

        // Wait for running or error state
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            match *self.state.read().await {
                SourceState::Running => return Ok(()),
                SourceState::Error(ref e) => return Err(SourceError::connection(e.clone())),
                _ => {}
            }
        }

        Err(SourceError::Timeout("start timed out".into()))
    }

    async fn stop(&mut self) -> SourceResult<()> {
        let _ = self.shutdown_tx.send(()).await;

        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if matches!(*self.state.read().await, SourceState::Stopped) {
                return Ok(());
            }
        }

        Err(SourceError::Timeout("stop timed out".into()))
    }

    async fn state(&self) -> SourceState {
        *self.state.read().await
    }

    async fn stats(&self) -> SourceStats {
        self.stats.to_source_stats(self.start_time)
    }

    fn subscribe(&self) -> mpsc::Receiver<TelemetryFrame> {
        let (tx, rx) = mpsc::channel(self.config.channel_capacity);
        let mut broadcast_rx = self.frame_tx.subscribe();

        tokio::spawn(async move {
            while let Ok(frame) = broadcast_rx.recv().await {
                if tx.send(frame).await.is_err() {
                    break;
                }
            }
        });

        rx
    }

    async fn configure(&mut self, config: SourceConfig) -> SourceResult<()> {
        if let Some(reconnect_str) = config.options.get("reconnect") {
            self.config.reconnect = reconnect_str == "true";
        }

        if let Some(delay_str) = config.options.get("reconnect_delay_ms") {
            if let Ok(ms) = delay_str.parse::<u64>() {
                self.config.reconnect_delay = Duration::from_millis(ms);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder() {
        let config = WsSourceConfig::new("ws://localhost:8080/telemetry")
            .with_source_id("test-ws")
            .with_reconnect(false)
            .with_channel_capacity(512);

        assert_eq!(config.url, "ws://localhost:8080/telemetry");
        assert_eq!(config.source_id, "test-ws");
        assert!(!config.reconnect);
        assert_eq!(config.channel_capacity, 512);
    }

    #[test]
    fn config_to_source_config() {
        let config = WsSourceConfig::new("ws://localhost:8080")
            .with_source_id("my-ws");

        let source_config: SourceConfig = config.into();
        assert_eq!(source_config.source_id, "my-ws");
        assert_eq!(source_config.options.get("url"), Some(&"ws://localhost:8080".to_string()));
    }

    #[test]
    fn decode_json_telemetry() {
        let json = r#"{"speed": 245.5, "rpm": 12500.0, "throttle": 0.85}"#;
        let mut seq = 0u64;
        
        let frame = AsyncWsSource::decode_message(json, "test", &mut seq);
        assert!(frame.is_some());
        assert_eq!(seq, 1);
        
        let frame = frame.unwrap();
        assert_eq!(frame.sample_count(), 3);
    }
}

//! Async UDP Telemetry Source
//!
//! This module provides an async UDP telemetry source that implements the
//! [`TelemetrySource`] trait from `pitgun-contract`.
//!
//! # Features
//!
//! - **Multi-codec support**: Auto-detect or explicitly select codec (ECUBridge, F1, etc.)
//! - **Multicast**: Join multicast groups for network telemetry
//! - **Async/await**: Non-blocking operation with tokio
//! - **Stats tracking**: Packets received, decoded, errors
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_source_udp::{AsyncUdpSource, UdpSourceConfig};
//! use pitgun_contract::TelemetrySource;
//!
//! let config = UdpSourceConfig::new("0.0.0.0:20777")
//!     .with_codec(UdpCodecType::F1);
//!
//! let source = AsyncUdpSource::new(config).await?;
//! source.start().await?;
//!
//! let mut rx = source.subscribe();
//! while let Ok(frame) = rx.recv().await {
//!     println!("Received frame: {} samples", frame.sample_count());
//! }
//! ```

use async_trait::async_trait;
use pitgun_contract::{
    CodecContext, DecodeOutput, SourceConfig, SourceError, SourceMetadata, SourceResult,
    SourceState, SourceStats, SourceType, TelemetryCodec, TelemetryFrame, TelemetrySource,
};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, RwLock};

/// Codec types supported by the UDP source
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UdpCodecType {
    /// Auto-detect codec based on packet format
    #[default]
    Auto,
    /// ECUBridge binary protocol
    EcuBridge,
    /// F1 UDP telemetry format
    F1,
    /// Legacy Pitgun v1 format
    PitgunV1,
}

impl UdpCodecType {
    /// Returns a human-readable name for the codec
    pub fn name(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::EcuBridge => "ecubridge",
            Self::F1 => "f1-udp",
            Self::PitgunV1 => "pitgun-v1",
        }
    }
}

/// Configuration for the async UDP source
#[derive(Clone, Debug)]
pub struct UdpSourceConfig {
    /// Bind address (e.g., "0.0.0.0:20777")
    pub bind_addr: SocketAddr,
    /// Multicast group to join (optional)
    pub multicast_group: Option<Ipv4Addr>,
    /// Interface for multicast (default: 0.0.0.0)
    pub multicast_interface: Ipv4Addr,
    /// Codec type to use
    pub codec: UdpCodecType,
    /// Source ID
    pub source_id: String,
    /// Buffer size for receive
    pub buffer_size: usize,
    /// Channel capacity for frame broadcasting
    pub channel_capacity: usize,
    /// Socket receive timeout
    pub recv_timeout: Option<Duration>,
}

impl UdpSourceConfig {
    /// Creates a new configuration with the given bind address
    pub fn new(bind_addr: impl Into<SocketAddr>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            multicast_group: None,
            multicast_interface: Ipv4Addr::UNSPECIFIED,
            codec: UdpCodecType::Auto,
            source_id: "udp-source".into(),
            buffer_size: 65536,
            channel_capacity: 1024,
            recv_timeout: None,
        }
    }

    /// Parses a bind address from string
    pub fn parse(addr: &str) -> Result<Self, std::net::AddrParseError> {
        let bind_addr: SocketAddr = addr.parse()?;
        Ok(Self::new(bind_addr))
    }

    /// Sets the codec type
    pub fn with_codec(mut self, codec: UdpCodecType) -> Self {
        self.codec = codec;
        self
    }

    /// Sets the source ID
    pub fn with_source_id(mut self, id: impl Into<String>) -> Self {
        self.source_id = id.into();
        self
    }

    /// Configures multicast group
    pub fn with_multicast(mut self, group: Ipv4Addr, interface: Ipv4Addr) -> Self {
        self.multicast_group = Some(group);
        self.multicast_interface = interface;
        self
    }

    /// Sets the receive buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Sets the channel capacity
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Sets the receive timeout
    pub fn with_recv_timeout(mut self, timeout: Duration) -> Self {
        self.recv_timeout = Some(timeout);
        self
    }
}

impl From<UdpSourceConfig> for SourceConfig {
    fn from(cfg: UdpSourceConfig) -> Self {
        let mut config = SourceConfig::new("udp", &cfg.source_id)
            .with_option("bind_addr", cfg.bind_addr.to_string())
            .with_option("codec", cfg.codec.name().to_string())
            .with_option("buffer_size", cfg.buffer_size.to_string())
            .with_option("channel_capacity", cfg.channel_capacity.to_string());

        if let Some(group) = cfg.multicast_group {
            config = config
                .with_option("multicast_group", group.to_string())
                .with_option("multicast_interface", cfg.multicast_interface.to_string());
        }

        if let Some(timeout) = cfg.recv_timeout {
            config = config.with_option("recv_timeout_ms", timeout.as_millis().to_string());
        }

        config
    }
}

/// ECUBridge standard buffer size (2MB)
pub const ECUBRIDGE_BUFFER_SIZE: usize = 2 * 1024 * 1024;

/// Sequence tracking for packet loss detection
#[derive(Debug, Default)]
struct SequenceTracker {
    last_sequence: AtomicU64,
    packets_lost: AtomicU64,
    out_of_order: AtomicU64,
    initialized: std::sync::atomic::AtomicBool,
}

impl SequenceTracker {
    fn track(&self, sequence: u64) {
        use std::sync::atomic::Ordering::Relaxed;
        
        if !self.initialized.load(Relaxed) {
            self.last_sequence.store(sequence, Relaxed);
            self.initialized.store(true, Relaxed);
            return;
        }
        
        let last = self.last_sequence.load(Relaxed);
        
        if sequence > last {
            // Calculate gap (missing packets)
            let gap = sequence - last - 1;
            if gap > 0 {
                self.packets_lost.fetch_add(gap, Relaxed);
            }
            self.last_sequence.store(sequence, Relaxed);
        } else if sequence < last {
            // Out of order packet
            self.out_of_order.fetch_add(1, Relaxed);
        }
        // sequence == last means duplicate, ignore
    }
    
    fn packets_lost(&self) -> u64 {
        self.packets_lost.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    fn out_of_order(&self) -> u64 {
        self.out_of_order.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Statistics for the UDP source
#[derive(Debug, Default)]
struct UdpStats {
    packets_received: AtomicU64,
    packets_decoded: AtomicU64,
    packets_skipped: AtomicU64,
    decode_errors: AtomicU64,
    bytes_received: AtomicU64,
    frames_produced: AtomicU64,
    samples_produced: AtomicU64,
    sequence_tracker: SequenceTracker,
}

impl UdpStats {
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
                    "packets_received".into(),
                    self.packets_received.load(Ordering::Relaxed) as f64,
                ),
                (
                    "packets_decoded".into(),
                    self.packets_decoded.load(Ordering::Relaxed) as f64,
                ),
                (
                    "packets_skipped".into(),
                    self.packets_skipped.load(Ordering::Relaxed) as f64,
                ),
                (
                    "packets_lost".into(),
                    self.sequence_tracker.packets_lost() as f64,
                ),
                (
                    "packets_out_of_order".into(),
                    self.sequence_tracker.out_of_order() as f64,
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
    
    fn track_sequence(&self, sequence: u64) {
        self.sequence_tracker.track(sequence);
    }
}

/// Async UDP telemetry source
pub struct AsyncUdpSource {
    config: UdpSourceConfig,
    state: Arc<RwLock<SourceState>>,
    stats: Arc<UdpStats>,
    start_time: Instant,
    frame_tx: broadcast::Sender<TelemetryFrame>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl AsyncUdpSource {
    /// Creates a new async UDP source
    pub fn new(config: UdpSourceConfig) -> Self {
        let (frame_tx, _) = broadcast::channel(config.channel_capacity);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Self {
            config,
            state: Arc::new(RwLock::new(SourceState::Stopped)),
            stats: Arc::new(UdpStats::default()),
            start_time: Instant::now(),
            frame_tx,
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
        }
    }

    /// Creates a new source and binds immediately
    pub async fn bind(config: UdpSourceConfig) -> SourceResult<Self> {
        // Validate the bind address by attempting to bind
        let socket = UdpSocket::bind(config.bind_addr)
            .await
            .map_err(|e| SourceError::connection(format!("failed to bind: {}", e)))?;

        // Join multicast if configured
        if let Some(group) = config.multicast_group {
            socket
                .join_multicast_v4(group, config.multicast_interface)
                .map_err(|e| SourceError::connection(format!("failed to join multicast: {}", e)))?;
        }

        drop(socket); // Close the test socket, will rebind in start()
        Ok(Self::new(config))
    }

    /// Detect codec from packet data
    fn detect_codec(&self, data: &[u8]) -> UdpCodecType {
        use pitgun_codec_udp::{EcuBridgeCodec, F1UdpCodec};
        use pitgun_contract::TelemetryCodec;

        let ecu = EcuBridgeCodec::new();
        if ecu.can_decode(data) {
            return UdpCodecType::EcuBridge;
        }

        let f1 = F1UdpCodec::new();
        if f1.can_decode(data) {
            return UdpCodecType::F1;
        }

        UdpCodecType::PitgunV1
    }

    /// Decode packet with the appropriate codec
    fn decode_packet(
        &self,
        data: &[u8],
        codec_type: UdpCodecType,
        ctx: &CodecContext,
    ) -> Result<DecodeOutput, pitgun_contract::CodecError> {
        use pitgun_codec_udp::{EcuBridgeCodec, F1UdpCodec};
        use pitgun_contract::TelemetryCodec;

        match codec_type {
            UdpCodecType::Auto => {
                let detected = self.detect_codec(data);
                self.decode_packet(data, detected, ctx)
            }
            UdpCodecType::EcuBridge => {
                let codec = EcuBridgeCodec::new();
                codec.decode(data, ctx)
            }
            UdpCodecType::F1 => {
                let codec = F1UdpCodec::new();
                codec.decode(data, ctx)
            }
            UdpCodecType::PitgunV1 => {
                // Legacy format - convert to TelemetryFrame
                use pitgun_codec_udp::{UdpDecoded, UdpDecoder, UdpWireFormat};
                use pitgun_contract::{Sample, SampleValue, SignalQuality, TelemetryFrameBuilder};

                let format = UdpWireFormat::PitgunV1;
                match format.decode(data) {
                    Ok(UdpDecoded::Events(events)) => {
                        let samples: Vec<Sample> = events
                            .iter()
                            .enumerate()
                            .map(|(i, e)| Sample {
                                parameter_id: i as u16,
                                value: SampleValue::F64(e.value),
                                quality: SignalQuality::Good,
                            })
                            .collect();

                        if samples.is_empty() {
                            return Ok(DecodeOutput::NoOutput);
                        }

                        let frame = TelemetryFrameBuilder::new()
                            .source_id(&ctx.source_id)
                            .samples(samples)
                            .build();

                        Ok(DecodeOutput::Frame(frame))
                    }
                    Ok(UdpDecoded::Batches(_)) => Ok(DecodeOutput::NoOutput),
                    Err(e) => Err(pitgun_contract::CodecError::MalformedData(e.to_string())),
                }
            }
        }
    }

    /// Run the receive loop
    async fn receive_loop(
        config: UdpSourceConfig,
        state: Arc<RwLock<SourceState>>,
        stats: Arc<UdpStats>,
        frame_tx: broadcast::Sender<TelemetryFrame>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        // Bind socket
        let socket = match UdpSocket::bind(config.bind_addr).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to bind UDP socket: {}", e);
                *state.write().await = SourceState::Error(e.to_string());
                return;
            }
        };

        // Join multicast
        if let Some(group) = config.multicast_group {
            if let Err(e) = socket.join_multicast_v4(group, config.multicast_interface) {
                eprintln!("Failed to join multicast: {}", e);
                *state.write().await = SourceState::Error(e.to_string());
                return;
            }
        }

        *state.write().await = SourceState::Running;

        let mut buf = vec![0u8; config.buffer_size];
        let ctx = CodecContext::new(1, &config.source_id);

        // For auto-detection, we'll lock onto the first detected codec
        let mut locked_codec: Option<UdpCodecType> = if config.codec == UdpCodecType::Auto {
            None
        } else {
            Some(config.codec)
        };

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                result = socket.recv(&mut buf) => {
                    match result {
                        Ok(n) => {
                            stats.packets_received.fetch_add(1, Ordering::Relaxed);
                            stats.bytes_received.fetch_add(n as u64, Ordering::Relaxed);

                            // Detect or use locked codec
                            let codec = locked_codec.unwrap_or_else(|| {
                                use pitgun_codec_udp::{EcuBridgeCodec, F1UdpCodec};
                                use pitgun_contract::TelemetryCodec;

                                let ecu = EcuBridgeCodec::new();
                                if ecu.can_decode(&buf[..n]) {
                                    return UdpCodecType::EcuBridge;
                                }
                                let f1 = F1UdpCodec::new();
                                if f1.can_decode(&buf[..n]) {
                                    return UdpCodecType::F1;
                                }
                                UdpCodecType::PitgunV1
                            });

                            // Lock onto detected codec
                            if locked_codec.is_none() {
                                locked_codec = Some(codec);
                            }

                            // Decode
                            let decode_result = {
                                use pitgun_codec_udp::{EcuBridgeCodec, F1UdpCodec};
                                use pitgun_contract::TelemetryCodec;

                                match codec {
                                    UdpCodecType::EcuBridge => {
                                        let c = EcuBridgeCodec::new();
                                        c.decode(&buf[..n], &ctx)
                                    }
                                    UdpCodecType::F1 => {
                                        let c = F1UdpCodec::new();
                                        c.decode(&buf[..n], &ctx)
                                    }
                                    _ => {
                                        // PitgunV1 or other
                                        use pitgun_codec_udp::{UdpDecoded, UdpDecoder, UdpWireFormat};
                                        use pitgun_contract::{Sample, SampleValue, SignalQuality, TelemetryFrameBuilder};

                                        let format = UdpWireFormat::PitgunV1;
                                        match format.decode(&buf[..n]) {
                                            Ok(UdpDecoded::Events(events)) => {
                                                let samples: Vec<Sample> = events
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(i, e)| Sample {
                                                        parameter_id: i as u16,
                                                        value: SampleValue::F64(e.value),
                                                        quality: SignalQuality::Good,
                                                    })
                                                    .collect();

                                                if samples.is_empty() {
                                                    Ok(DecodeOutput::NoOutput)
                                                } else {
                                                    let frame = TelemetryFrameBuilder::new()
                                                        .source_id(&ctx.source_id)
                                                        .samples(samples)
                                                        .build();
                                                    Ok(DecodeOutput::Frame(frame))
                                                }
                                            }
                                            Ok(UdpDecoded::Batches(_)) => Ok(DecodeOutput::NoOutput),
                                            Err(e) => Err(pitgun_contract::CodecError::MalformedData(e.to_string())),
                                        }
                                    }
                                }
                            };

                            match decode_result {
                                Ok(DecodeOutput::Frame(frame)) => {
                                    stats.packets_decoded.fetch_add(1, Ordering::Relaxed);
                                    stats.frames_produced.fetch_add(1, Ordering::Relaxed);
                                    stats.samples_produced.fetch_add(frame.sample_count() as u64, Ordering::Relaxed);
                                    
                                    // Track sequence for packet loss detection
                                    stats.track_sequence(frame.sequence());

                                    // Broadcast frame (ignore if no receivers)
                                    let _ = frame_tx.send(frame);
                                }
                                Ok(DecodeOutput::Frames(frames)) => {
                                    stats.packets_decoded.fetch_add(1, Ordering::Relaxed);
                                    for frame in frames {
                                        stats.frames_produced.fetch_add(1, Ordering::Relaxed);
                                        stats.samples_produced.fetch_add(frame.sample_count() as u64, Ordering::Relaxed);
                                        let _ = frame_tx.send(frame);
                                    }
                                }
                                Ok(DecodeOutput::Skipped(_)) => {
                                    stats.packets_skipped.fetch_add(1, Ordering::Relaxed);
                                }
                                Ok(DecodeOutput::NoOutput) => {
                                    stats.packets_skipped.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(_) => {
                                    stats.decode_errors.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("UDP receive error: {}", e);
                            stats.decode_errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }

        *state.write().await = SourceState::Stopped;
    }
}

#[async_trait]
impl TelemetrySource for AsyncUdpSource {
    fn name(&self) -> &str {
        &self.config.source_id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Udp
    }

    fn metadata(&self) -> SourceMetadata {
        SourceMetadata {
            id: self.config.source_id.clone(),
            name: format!("UDP Source ({})", self.config.bind_addr),
            source_type: SourceType::Udp,
            version: Some(env!("CARGO_PKG_VERSION").into()),
            description: Some(format!(
                "UDP telemetry source listening on {} with {} codec",
                self.config.bind_addr,
                self.config.codec.name()
            )),
            capabilities: vec!["decode".into(), "multicast".into()],
            tags: vec!["udp".into(), self.config.codec.name().into()],
        }
    }

    async fn start(&mut self) -> SourceResult<()> {
        let current_state = *self.state.read().await;
        if matches!(current_state, SourceState::Running) {
            return Err(SourceError::invalid_state("already running"));
        }

        // Take the shutdown receiver
        let shutdown_rx = self
            .shutdown_rx
            .take()
            .ok_or_else(|| SourceError::invalid_state("source already started"))?;

        *self.state.write().await = SourceState::Starting;
        self.start_time = Instant::now();

        // Spawn receive loop
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let stats = Arc::clone(&self.stats);
        let frame_tx = self.frame_tx.clone();

        tokio::spawn(async move {
            Self::receive_loop(config, state, stats, frame_tx, shutdown_rx).await;
        });

        // Wait for running state
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if matches!(*self.state.read().await, SourceState::Running) {
                return Ok(());
            }
            if let SourceState::Error(ref e) = *self.state.read().await {
                return Err(SourceError::connection(e.clone()));
            }
        }

        Err(SourceError::Timeout("start timed out".into()))
    }

    async fn stop(&mut self) -> SourceResult<()> {
        let _ = self.shutdown_tx.send(()).await;

        // Wait for stopped state
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

    fn subscribe(&self) -> tokio::sync::mpsc::Receiver<TelemetryFrame> {
        let (tx, rx) = tokio::sync::mpsc::channel(self.config.channel_capacity);
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
        // Parse configuration options
        if let Some(codec_str) = config.options.get("codec") {
            self.config.codec = match codec_str.as_str() {
                "ecubridge" => UdpCodecType::EcuBridge,
                "f1-udp" | "f1" => UdpCodecType::F1,
                "pitgun-v1" | "pitgun" => UdpCodecType::PitgunV1,
                "auto" => UdpCodecType::Auto,
                _ => return Err(SourceError::config(format!("unknown codec: {}", codec_str))),
            };
        }

        if let Some(buf_size) = config.options.get("buffer_size") {
            self.config.buffer_size = buf_size
                .parse()
                .map_err(|_| SourceError::config("invalid buffer_size"))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder() {
        let config = UdpSourceConfig::new("0.0.0.0:20777".parse::<SocketAddr>().unwrap())
            .with_codec(UdpCodecType::F1)
            .with_source_id("test-source")
            .with_buffer_size(32768);

        assert_eq!(config.codec, UdpCodecType::F1);
        assert_eq!(config.source_id, "test-source");
        assert_eq!(config.buffer_size, 32768);
    }

    #[test]
    fn config_parse() {
        let config = UdpSourceConfig::parse("0.0.0.0:20777").unwrap();
        assert_eq!(config.bind_addr.port(), 20777);
    }

    #[test]
    fn codec_type_names() {
        assert_eq!(UdpCodecType::Auto.name(), "auto");
        assert_eq!(UdpCodecType::EcuBridge.name(), "ecubridge");
        assert_eq!(UdpCodecType::F1.name(), "f1-udp");
        assert_eq!(UdpCodecType::PitgunV1.name(), "pitgun-v1");
    }

    #[test]
    fn config_to_source_config() {
        let config = UdpSourceConfig::new("0.0.0.0:20777".parse::<SocketAddr>().unwrap())
            .with_codec(UdpCodecType::F1)
            .with_source_id("my-source");

        let source_config: SourceConfig = config.into();
        assert_eq!(source_config.source_id, "my-source");
        assert_eq!(source_config.options.get("codec"), Some(&"f1-udp".to_string()));
    }
}

//! Async Physics Telemetry Source
//!
//! This module provides an async physics simulation source that implements the
//! [`TelemetrySource`] trait from `pitgun-contract`.
//!
//! # Features
//!
//! - **Deterministic simulation**: Reproducible physics output for testing
//! - **Configurable frame rate**: Control output frequency
//! - **Async/await**: Non-blocking operation with tokio
//! - **Stats tracking**: Frames produced, simulation time
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_source_physics::{AsyncPhysicsSource, AsyncPhysicsConfig};
//! use pitgun_contract::TelemetrySource;
//!
//! let config = AsyncPhysicsConfig::new()
//!     .with_tick_hz(60)
//!     .with_source_id("physics");
//!
//! let mut source = AsyncPhysicsSource::new(config);
//! source.start().await?;
//!
//! let mut rx = source.subscribe();
//! while let Some(frame) = rx.recv().await {
//!     println!("Physics frame: {} samples", frame.sample_count());
//! }
//! ```

use async_trait::async_trait;
use pitgun_contract::{
    Sample, SampleValue, SignalQuality, SourceConfig, SourceError, SourceMetadata, SourceResult,
    SourceState, SourceStats, SourceType, TelemetryFrame, TelemetryFrameBuilder, TelemetrySource,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::{PhysicsSource, PhysicsSourceConfig};

/// Parameter IDs for physics channels (matching f1_generic.yaml)
pub mod param_ids {
    pub const SPEED_KPH: u16 = 40;
    pub const RPM: u16 = 1;
    pub const GEAR_INDEX: u16 = 30;
    pub const THROTTLE_PCT: u16 = 10;
    pub const BRAKE_PCT: u16 = 11;
    pub const STEERING_ANGLE_DEG: u16 = 20;
    pub const G_LAT: u16 = 50;
    pub const G_LONG: u16 = 51;
    pub const ENGINE_TEMP_C: u16 = 2;
    pub const DRAG_N: u16 = 200;
    pub const DOWNFORCE_N: u16 = 201;
    pub const INSTABILITY_INDEX: u16 = 202;
    pub const BOOST_PRESSURE_BAR: u16 = 203;
}

/// Configuration for the async physics source
#[derive(Clone, Debug)]
pub struct AsyncPhysicsConfig {
    /// Physics configuration
    pub physics: PhysicsSourceConfig,
    /// Source ID
    pub source_id: String,
    /// Channel capacity for frame broadcasting
    pub channel_capacity: usize,
    /// Whether to emit in real-time or as fast as possible
    pub real_time: bool,
}

impl Default for AsyncPhysicsConfig {
    fn default() -> Self {
        Self {
            physics: PhysicsSourceConfig::default(),
            source_id: "physics-source".into(),
            channel_capacity: 1024,
            real_time: true,
        }
    }
}

impl AsyncPhysicsConfig {
    /// Creates a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the tick rate in Hz
    pub fn with_tick_hz(mut self, hz: u32) -> Self {
        self.physics.tick_hz = hz;
        self
    }

    /// Sets the source ID
    pub fn with_source_id(mut self, id: impl Into<String>) -> Self {
        self.source_id = id.into();
        self
    }

    /// Sets the duration in ticks
    pub fn with_duration_ticks(mut self, ticks: u64) -> Self {
        self.physics.duration_ticks = ticks;
        self
    }

    /// Sets the batch size in ticks
    pub fn with_batch_ticks(mut self, ticks: u32) -> Self {
        self.physics.batch_ticks = ticks;
        self
    }

    /// Sets the channel capacity
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Sets real-time mode (default: true)
    pub fn with_real_time(mut self, real_time: bool) -> Self {
        self.real_time = real_time;
        self
    }

    /// Uses the underlying physics config directly
    pub fn with_physics_config(mut self, config: PhysicsSourceConfig) -> Self {
        self.physics = config;
        self
    }
}

impl From<AsyncPhysicsConfig> for SourceConfig {
    fn from(cfg: AsyncPhysicsConfig) -> Self {
        SourceConfig::new("physics", &cfg.source_id)
            .with_option("tick_hz", cfg.physics.tick_hz.to_string())
            .with_option("duration_ticks", cfg.physics.duration_ticks.to_string())
            .with_option("batch_ticks", cfg.physics.batch_ticks.to_string())
            .with_option("real_time", cfg.real_time.to_string())
            .with_option("channel_capacity", cfg.channel_capacity.to_string())
    }
}

/// Statistics for the physics source
#[derive(Debug, Default)]
struct PhysicsStats {
    ticks_produced: AtomicU64,
    frames_produced: AtomicU64,
    samples_produced: AtomicU64,
}

impl PhysicsStats {
    fn to_source_stats(&self, start_time: Instant, tick_hz: u32) -> SourceStats {
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
            bytes_processed: 0, // Physics doesn't process bytes
            errors: 0,
            sample_rate_hz: rate,
            uptime: start_time.elapsed(),
            custom: [
                (
                    "ticks_produced".into(),
                    self.ticks_produced.load(Ordering::Relaxed) as f64,
                ),
                ("tick_hz".into(), tick_hz as f64),
            ]
            .into_iter()
            .collect(),
        }
    }
}

/// Async physics telemetry source
pub struct AsyncPhysicsSource {
    config: AsyncPhysicsConfig,
    state: Arc<RwLock<SourceState>>,
    stats: Arc<PhysicsStats>,
    start_time: Instant,
    frame_tx: broadcast::Sender<TelemetryFrame>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl AsyncPhysicsSource {
    /// Creates a new async physics source
    pub fn new(config: AsyncPhysicsConfig) -> Self {
        let (frame_tx, _) = broadcast::channel(config.channel_capacity);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Self {
            config,
            state: Arc::new(RwLock::new(SourceState::Stopped)),
            stats: Arc::new(PhysicsStats::default()),
            start_time: Instant::now(),
            frame_tx,
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
        }
    }

    /// Convert physics batch to TelemetryFrame
    fn batch_to_frame(
        batch: &pitgun_core::EventBatch,
        source_id: &str,
        sequence: u64,
    ) -> TelemetryFrame {
        let samples: Vec<Sample> = batch
            .events
            .iter()
            .map(|e| {
                let param_id = match e.channel.as_str() {
                    "speed_kph" => param_ids::SPEED_KPH,
                    "rpm" => param_ids::RPM,
                    "gear_index" => param_ids::GEAR_INDEX,
                    "throttle_pct" => param_ids::THROTTLE_PCT,
                    "brake_pct" => param_ids::BRAKE_PCT,
                    "steering_angle_deg" => param_ids::STEERING_ANGLE_DEG,
                    "g_lat" => param_ids::G_LAT,
                    "g_long" => param_ids::G_LONG,
                    "engine_temp_c" => param_ids::ENGINE_TEMP_C,
                    "current_drag_n" => param_ids::DRAG_N,
                    "current_downforce_n" => param_ids::DOWNFORCE_N,
                    "instability_index" => param_ids::INSTABILITY_INDEX,
                    "boost_pressure_bar" => param_ids::BOOST_PRESSURE_BAR,
                    _ => 0,
                };
                Sample {
                    parameter_id: param_id,
                    value: SampleValue::F64(e.value),
                    quality: SignalQuality::Good,
                }
            })
            .collect();

        // Use first event's timestamp if available
        let timestamp_us = batch
            .events
            .first()
            .map(|e| (e.ts_ns / 1000) as i64)
            .unwrap_or(0);

        TelemetryFrameBuilder::new()
            .source_id(source_id)
            .sequence(sequence)
            .timestamp_us(timestamp_us)
            .samples(samples)
            .build()
    }

    /// Run the physics simulation loop
    async fn simulation_loop(
        config: AsyncPhysicsConfig,
        state: Arc<RwLock<SourceState>>,
        stats: Arc<PhysicsStats>,
        frame_tx: broadcast::Sender<TelemetryFrame>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        use pitgun_core::Source;

        let mut physics = PhysicsSource::new(config.physics.clone());
        let tick_duration = Duration::from_secs_f64(1.0 / config.physics.tick_hz as f64);
        let batch_duration = tick_duration * config.physics.batch_ticks;

        *state.write().await = SourceState::Running;

        let mut sequence = 0u64;
        let mut next_tick = Instant::now();

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                _ = async {
                    if config.real_time {
                        tokio::time::sleep_until(tokio::time::Instant::from_std(next_tick)).await;
                    }
                } => {}
            }

            // Generate physics batch
            let batch = match physics.next_batch() {
                Some(b) => b,
                None => {
                    // Simulation complete
                    break;
                }
            };

            if batch.end_of_stream {
                break;
            }

            sequence += 1;
            let frame = Self::batch_to_frame(&batch, &config.source_id, sequence);

            stats
                .ticks_produced
                .fetch_add(config.physics.batch_ticks as u64, Ordering::Relaxed);
            stats.frames_produced.fetch_add(1, Ordering::Relaxed);
            stats
                .samples_produced
                .fetch_add(frame.sample_count() as u64, Ordering::Relaxed);

            // Broadcast frame
            let _ = frame_tx.send(frame);

            if config.real_time {
                next_tick += batch_duration;
            }
        }

        *state.write().await = SourceState::Stopped;
    }
}

#[async_trait]
impl TelemetrySource for AsyncPhysicsSource {
    fn name(&self) -> &str {
        &self.config.source_id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Physics
    }

    fn metadata(&self) -> SourceMetadata {
        SourceMetadata {
            id: self.config.source_id.clone(),
            name: format!("Physics Source ({}Hz)", self.config.physics.tick_hz),
            source_type: SourceType::Physics,
            version: Some(env!("CARGO_PKG_VERSION").into()),
            description: Some(format!(
                "Deterministic physics simulation at {}Hz",
                self.config.physics.tick_hz
            )),
            capabilities: vec!["deterministic".into(), "simulation".into()],
            tags: vec!["physics".into(), "simulation".into()],
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
            Self::simulation_loop(config, state, stats, frame_tx, shutdown_rx).await;
        });

        // Wait for running state
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if matches!(*self.state.read().await, SourceState::Running) {
                return Ok(());
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
        self.stats
            .to_source_stats(self.start_time, self.config.physics.tick_hz)
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
        if let Some(tick_hz_str) = config.options.get("tick_hz") {
            self.config.physics.tick_hz = tick_hz_str
                .parse()
                .map_err(|_| SourceError::config("invalid tick_hz"))?;
        }

        if let Some(real_time_str) = config.options.get("real_time") {
            self.config.real_time = real_time_str == "true";
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder() {
        let config = AsyncPhysicsConfig::new()
            .with_tick_hz(120)
            .with_source_id("test-physics")
            .with_real_time(false);

        assert_eq!(config.physics.tick_hz, 120);
        assert_eq!(config.source_id, "test-physics");
        assert!(!config.real_time);
    }

    #[test]
    fn config_to_source_config() {
        let config = AsyncPhysicsConfig::new()
            .with_tick_hz(60)
            .with_source_id("my-physics");

        let source_config: SourceConfig = config.into();
        assert_eq!(source_config.source_id, "my-physics");
        assert_eq!(source_config.options.get("tick_hz"), Some(&"60".to_string()));
    }
}

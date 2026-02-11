//! Multi-Source Telemetry Pipeline
//!
//! This module provides the core pipeline for aggregating telemetry from multiple
//! sources into a unified processing flow.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │ UDP Source  │────▶│             │     │             │
//! └─────────────┘     │             │     │             │
//! ┌─────────────┐     │   Frame     │────▶│  Processor  │────▶ Output
//! │ WS Source   │────▶│   Merger    │     │   Chain     │
//! └─────────────┘     │             │     │             │
//! ┌─────────────┐     │             │     │             │
//! │ Kafka Source│────▶│             │     │             │
//! └─────────────┘     └─────────────┘     └─────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_core::pipeline::{TelemetryPipeline, PipelineConfig};
//! use pitgun_contract::TelemetrySource;
//!
//! let mut pipeline = TelemetryPipeline::new(PipelineConfig::default());
//!
//! pipeline.add_source(udp_source);
//! pipeline.add_source(ws_source);
//!
//! pipeline.start().await?;
//!
//! let mut rx = pipeline.subscribe();
//! while let Some(frame) = rx.recv().await {
//!     println!("Merged frame: {} samples", frame.sample_count());
//! }
//! ```

use async_trait::async_trait;
use pitgun_contract::{
    ParameterRegistry, SourceError, SourceResult, SourceState, SourceStats, TelemetryFrame,
    TelemetrySource,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast, mpsc};

/// Pipeline configuration
#[derive(Clone, Debug)]
pub struct PipelineConfig {
    /// Channel capacity for merged frames
    pub channel_capacity: usize,
    /// Maximum sources allowed
    pub max_sources: usize,
    /// Enable frame merging (combine frames from same timestamp)
    pub enable_merging: bool,
    /// Merge window duration (frames within this window are merged)
    pub merge_window: Duration,
    /// Enable parameter validation against registry
    pub validate_parameters: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 4096,
            max_sources: 16,
            enable_merging: false,
            merge_window: Duration::from_millis(1),
            validate_parameters: false,
        }
    }
}

impl PipelineConfig {
    /// Creates a new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the channel capacity
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Enables frame merging
    pub fn with_merging(mut self, window: Duration) -> Self {
        self.enable_merging = true;
        self.merge_window = window;
        self
    }

    /// Enables parameter validation
    pub fn with_validation(mut self) -> Self {
        self.validate_parameters = true;
        self
    }
}

/// Pipeline statistics
#[derive(Debug, Default)]
pub struct PipelineStats {
    frames_received: AtomicU64,
    frames_merged: AtomicU64,
    frames_output: AtomicU64,
    samples_processed: AtomicU64,
    source_errors: AtomicU64,
}

impl PipelineStats {
    /// Converts to a HashMap for reporting
    pub fn to_map(&self, start_time: Instant) -> HashMap<String, f64> {
        let elapsed = start_time.elapsed().as_secs_f64();
        let frames = self.frames_output.load(Ordering::Relaxed);
        let rate = if elapsed > 0.0 {
            frames as f64 / elapsed
        } else {
            0.0
        };

        [
            (
                "frames_received".into(),
                self.frames_received.load(Ordering::Relaxed) as f64,
            ),
            (
                "frames_merged".into(),
                self.frames_merged.load(Ordering::Relaxed) as f64,
            ),
            ("frames_output".into(), frames as f64),
            (
                "samples_processed".into(),
                self.samples_processed.load(Ordering::Relaxed) as f64,
            ),
            (
                "source_errors".into(),
                self.source_errors.load(Ordering::Relaxed) as f64,
            ),
            ("output_rate_hz".into(), rate),
            ("uptime_secs".into(), elapsed),
        ]
        .into_iter()
        .collect()
    }
}

/// Source handle for tracking individual sources
struct SourceHandle {
    name: String,
    source: Box<dyn TelemetrySource + Send>,
    receiver: Option<mpsc::UnboundedReceiver<TelemetryFrame>>,
}

/// Multi-source telemetry pipeline
pub struct TelemetryPipeline {
    config: PipelineConfig,
    sources: Vec<SourceHandle>,
    registry: Option<Arc<ParameterRegistry>>,
    state: Arc<RwLock<SourceState>>,
    stats: Arc<PipelineStats>,
    start_time: Instant,
    output_tx: broadcast::Sender<TelemetryFrame>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl TelemetryPipeline {
    /// Creates a new pipeline with the given configuration
    pub fn new(config: PipelineConfig) -> Self {
        let (output_tx, _) = broadcast::channel(config.channel_capacity);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Self {
            config,
            sources: Vec::new(),
            registry: None,
            state: Arc::new(RwLock::new(SourceState::Stopped)),
            stats: Arc::new(PipelineStats::default()),
            start_time: Instant::now(),
            output_tx,
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
        }
    }

    /// Creates a pipeline with a parameter registry for validation
    pub fn with_registry(config: PipelineConfig, registry: Arc<ParameterRegistry>) -> Self {
        let mut pipeline = Self::new(config);
        pipeline.registry = Some(registry);
        pipeline
    }

    /// Adds a telemetry source to the pipeline
    pub fn add_source<S>(&mut self, name: impl Into<String>, source: S) -> SourceResult<()>
    where
        S: TelemetrySource + Send + 'static,
    {
        self.add_source_boxed(name, Box::new(source))
    }

    /// Adds a boxed telemetry source to the pipeline
    pub fn add_source_boxed(
        &mut self,
        name: impl Into<String>,
        source: Box<dyn TelemetrySource + Send>,
    ) -> SourceResult<()> {
        if self.sources.len() >= self.config.max_sources {
            return Err(SourceError::InvalidConfig(format!(
                "maximum sources ({}) exceeded",
                self.config.max_sources
            )));
        }

        self.sources.push(SourceHandle {
            name: name.into(),
            source,
            receiver: None,
        });

        Ok(())
    }

    /// Returns the number of configured sources
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Returns the names of all configured sources
    pub fn source_names(&self) -> Vec<String> {
        self.sources.iter().map(|s| s.name.clone()).collect()
    }

    /// Gets statistics for a specific source
    pub fn source_stats(&self, name: &str) -> Option<SourceStats> {
        for handle in &self.sources {
            if handle.name == name {
                return Some(handle.source.stats());
            }
        }
        None
    }

    /// Gets pipeline statistics
    pub fn pipeline_stats(&self) -> HashMap<String, f64> {
        self.stats.to_map(self.start_time)
    }

    /// Subscribes to the merged output stream
    pub fn subscribe(&self) -> mpsc::Receiver<TelemetryFrame> {
        let (tx, rx) = mpsc::channel(self.config.channel_capacity);
        let mut broadcast_rx = self.output_tx.subscribe();

        tokio::spawn(async move {
            while let Ok(frame) = broadcast_rx.recv().await {
                if tx.send(frame).await.is_err() {
                    break;
                }
            }
        });

        rx
    }

    /// Starts all sources and begins processing
    pub async fn start(&mut self) -> SourceResult<()> {
        let current_state = *self.state.read().await;
        if matches!(current_state, SourceState::Running) {
            return Err(SourceError::AlreadyRunning);
        }

        if self.sources.is_empty() {
            return Err(SourceError::InvalidConfig("no sources configured".into()));
        }

        *self.state.write().await = SourceState::Connecting;
        self.start_time = Instant::now();

        // Start all sources and collect their receivers
        for handle in &mut self.sources {
            let (tx, rx) = mpsc::unbounded_channel();
            handle.source.start(tx).await?;
            handle.receiver = Some(rx);
        }

        // Take the shutdown receiver
        let shutdown_rx = self.shutdown_rx.take().ok_or(SourceError::AlreadyRunning)?;

        // Spawn the merge loop
        let stats = Arc::clone(&self.stats);
        let state = Arc::clone(&self.state);
        let output_tx = self.output_tx.clone();
        let enable_merging = self.config.enable_merging;

        // Collect receivers
        let receivers: Vec<_> = self
            .sources
            .iter_mut()
            .filter_map(|h| h.receiver.take())
            .collect();

        tokio::spawn(async move {
            Self::merge_loop(
                receivers,
                output_tx,
                stats,
                state,
                shutdown_rx,
                enable_merging,
            )
            .await;
        });

        // Wait for running state
        *self.state.write().await = SourceState::Running;
        Ok(())
    }

    /// Stops all sources and the pipeline
    pub async fn stop(&mut self) -> SourceResult<()> {
        // Signal shutdown
        let _ = self.shutdown_tx.send(()).await;

        // Stop all sources
        for handle in &mut self.sources {
            let _ = handle.source.stop().await;
        }

        // Wait for stopped state
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if matches!(*self.state.read().await, SourceState::Stopped) {
                return Ok(());
            }
        }

        *self.state.write().await = SourceState::Stopped;
        Ok(())
    }

    /// Returns the current pipeline state
    pub async fn state(&self) -> SourceState {
        *self.state.read().await
    }

    /// The merge loop that combines frames from all sources
    async fn merge_loop(
        mut receivers: Vec<mpsc::UnboundedReceiver<TelemetryFrame>>,
        output_tx: broadcast::Sender<TelemetryFrame>,
        stats: Arc<PipelineStats>,
        state: Arc<RwLock<SourceState>>,
        mut shutdown_rx: mpsc::Receiver<()>,
        _enable_merging: bool,
    ) {
        use futures::stream::{FuturesUnordered, StreamExt};

        loop {
            // Create futures for all receivers
            let mut futures = FuturesUnordered::new();

            for (idx, rx) in receivers.iter_mut().enumerate() {
                futures.push(async move { (idx, rx.recv().await) });
            }

            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                result = futures.next() => {
                    match result {
                        Some((_idx, Some(frame))) => {
                            stats.frames_received.fetch_add(1, Ordering::Relaxed);
                            stats.samples_processed.fetch_add(frame.sample_count() as u64, Ordering::Relaxed);
                            stats.frames_output.fetch_add(1, Ordering::Relaxed);

                            // For now, pass through without merging
                            // TODO: Implement frame merging based on timestamp
                            let _ = output_tx.send(frame);
                        }
                        Some((_idx, None)) => {
                            // Source closed
                            stats.source_errors.fetch_add(1, Ordering::Relaxed);
                        }
                        None => {
                            // All futures completed (shouldn't happen in loop)
                            break;
                        }
                    }
                }
            }
        }

        *state.write().await = SourceState::Stopped;
    }
}

/// Frame processor trait for the pipeline
#[async_trait]
pub trait FrameProcessor: Send + Sync {
    /// Process a frame and optionally transform it
    async fn process(&self, frame: TelemetryFrame) -> Option<TelemetryFrame>;

    /// Returns the processor name
    fn name(&self) -> &str;
}

/// Validation processor that checks parameters against registry
pub struct ValidationProcessor {
    registry: Arc<ParameterRegistry>,
}

impl ValidationProcessor {
    pub fn new(registry: Arc<ParameterRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl FrameProcessor for ValidationProcessor {
    async fn process(&self, frame: TelemetryFrame) -> Option<TelemetryFrame> {
        // Validate each sample's parameter_id exists in registry
        for sample in &frame.samples {
            if self.registry.get(sample.parameter_id).is_none() {
                // Unknown parameter - could log or filter
            }
        }
        Some(frame)
    }

    fn name(&self) -> &str {
        "validation"
    }
}

/// Logging processor for debugging
pub struct LoggingProcessor {
    prefix: String,
}

impl LoggingProcessor {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

#[async_trait]
impl FrameProcessor for LoggingProcessor {
    async fn process(&self, frame: TelemetryFrame) -> Option<TelemetryFrame> {
        eprintln!(
            "{}: seq={} samples={} source={}",
            self.prefix,
            frame.sequence,
            frame.sample_count(),
            &frame.source_id
        );
        Some(frame)
    }

    fn name(&self) -> &str {
        "logging"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_config_builder() {
        let config = PipelineConfig::new()
            .with_channel_capacity(2048)
            .with_merging(Duration::from_millis(5))
            .with_validation();

        assert_eq!(config.channel_capacity, 2048);
        assert!(config.enable_merging);
        assert_eq!(config.merge_window, Duration::from_millis(5));
        assert!(config.validate_parameters);
    }

    #[test]
    fn pipeline_stats() {
        let stats = PipelineStats::default();
        stats.frames_received.fetch_add(100, Ordering::Relaxed);
        stats.frames_output.fetch_add(95, Ordering::Relaxed);

        let map = stats.to_map(Instant::now());
        assert_eq!(map.get("frames_received"), Some(&100.0));
        assert_eq!(map.get("frames_output"), Some(&95.0));
    }
}

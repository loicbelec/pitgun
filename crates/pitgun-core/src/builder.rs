//! Pipeline Builder with Fluent API
//!
//! Provides a convenient builder pattern for constructing telemetry pipelines
//! with multiple sources.
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_core::builder::PipelineBuilder;
//!
//! let pipeline = PipelineBuilder::new()
//!     .with_udp("0.0.0.0:20777", "ecubridge")
//!     .with_websocket("ws://localhost:8080/telemetry")
//!     .with_kafka(&["broker:9092"], "telemetry-topic", "my-group")
//!     .with_merging(Duration::from_millis(2))
//!     .build()
//!     .await?;
//! ```

use crate::pipeline::{PipelineConfig, TelemetryPipeline};
use pitgun_contract::{ParameterRegistry, SourceError, SourceResult};
use std::sync::Arc;
use std::time::Duration;

/// Source configuration for deferred source creation
#[derive(Clone, Debug)]
pub enum SourceConfig {
    /// UDP source configuration
    Udp { bind_addr: String, codec: String },
    /// WebSocket source configuration  
    WebSocket { url: String, codec: Option<String> },
    /// Kafka source configuration
    Kafka {
        brokers: Vec<String>,
        topic: String,
        group_id: String,
    },
    /// MQTT source configuration
    Mqtt {
        url: String,
        topic: String,
        client_id: String,
    },
    /// Physics simulation source
    Physics { model_name: String, frame_rate: f64 },
}

/// Builder for constructing TelemetryPipeline instances
pub struct PipelineBuilder {
    config: PipelineConfig,
    sources: Vec<(String, SourceConfig)>,
    registry: Option<Arc<ParameterRegistry>>,
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineBuilder {
    /// Creates a new pipeline builder with default configuration
    pub fn new() -> Self {
        Self {
            config: PipelineConfig::default(),
            sources: Vec::new(),
            registry: None,
        }
    }

    /// Sets a custom pipeline configuration
    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the channel capacity for frame buffering
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.config.channel_capacity = capacity;
        self
    }

    /// Enables frame merging with the given window
    pub fn with_merging(mut self, window: Duration) -> Self {
        self.config.enable_merging = true;
        self.config.merge_window = window;
        self
    }

    /// Enables parameter validation against a registry
    pub fn with_validation(mut self, registry: Arc<ParameterRegistry>) -> Self {
        self.config.validate_parameters = true;
        self.registry = Some(registry);
        self
    }

    /// Adds a UDP source with the specified bind address and codec
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this source
    /// * `bind_addr` - Socket address to bind (e.g., "0.0.0.0:20777")
    /// * `codec` - Codec name (e.g., "ecubridge", "f1")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_udp("ecu", "0.0.0.0:20777", "ecubridge")
    /// ```
    pub fn with_udp(
        mut self,
        name: impl Into<String>,
        bind_addr: impl Into<String>,
        codec: impl Into<String>,
    ) -> Self {
        self.sources.push((
            name.into(),
            SourceConfig::Udp {
                bind_addr: bind_addr.into(),
                codec: codec.into(),
            },
        ));
        self
    }

    /// Adds a WebSocket source
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this source
    /// * `url` - WebSocket URL (e.g., "ws://localhost:8080/telemetry")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_websocket("ws", "ws://localhost:8080/telemetry")
    /// ```
    pub fn with_websocket(mut self, name: impl Into<String>, url: impl Into<String>) -> Self {
        self.sources.push((
            name.into(),
            SourceConfig::WebSocket {
                url: url.into(),
                codec: None,
            },
        ));
        self
    }

    /// Adds a WebSocket source with a specific codec
    pub fn with_websocket_codec(
        mut self,
        name: impl Into<String>,
        url: impl Into<String>,
        codec: impl Into<String>,
    ) -> Self {
        self.sources.push((
            name.into(),
            SourceConfig::WebSocket {
                url: url.into(),
                codec: Some(codec.into()),
            },
        ));
        self
    }

    /// Adds a Kafka source
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this source
    /// * `brokers` - List of Kafka broker addresses
    /// * `topic` - Topic to subscribe to
    /// * `group_id` - Consumer group ID
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_kafka("kafka", &["localhost:9092"], "telemetry", "pitgun-group")
    /// ```
    pub fn with_kafka<S: AsRef<str>>(
        mut self,
        name: impl Into<String>,
        brokers: &[S],
        topic: impl Into<String>,
        group_id: impl Into<String>,
    ) -> Self {
        self.sources.push((
            name.into(),
            SourceConfig::Kafka {
                brokers: brokers.iter().map(|s| s.as_ref().to_string()).collect(),
                topic: topic.into(),
                group_id: group_id.into(),
            },
        ));
        self
    }

    /// Adds an MQTT source
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this source
    /// * `url` - MQTT broker URL (e.g., "mqtt://localhost:1883")
    /// * `topic` - Topic pattern to subscribe to (supports wildcards)
    /// * `client_id` - Client identifier
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_mqtt("mqtt", "mqtt://localhost:1883", "telemetry/#", "pitgun-client")
    /// ```
    pub fn with_mqtt(
        mut self,
        name: impl Into<String>,
        url: impl Into<String>,
        topic: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Self {
        self.sources.push((
            name.into(),
            SourceConfig::Mqtt {
                url: url.into(),
                topic: topic.into(),
                client_id: client_id.into(),
            },
        ));
        self
    }

    /// Adds a physics simulation source
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this source
    /// * `model_name` - Physics model identifier
    /// * `frame_rate` - Simulation frame rate in Hz
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_physics("physics", "default", 1000.0)
    /// ```
    pub fn with_physics(
        mut self,
        name: impl Into<String>,
        model_name: impl Into<String>,
        frame_rate: f64,
    ) -> Self {
        self.sources.push((
            name.into(),
            SourceConfig::Physics {
                model_name: model_name.into(),
                frame_rate,
            },
        ));
        self
    }

    /// Returns the number of configured sources
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Returns the source configurations for inspection
    pub fn sources(&self) -> &[(String, SourceConfig)] {
        &self.sources
    }

    /// Builds the pipeline configuration without creating sources
    ///
    /// This is useful when you want to inspect the configuration
    /// before building the actual pipeline.
    pub fn into_config(self) -> (PipelineConfig, Vec<(String, SourceConfig)>) {
        (self.config, self.sources)
    }

    /// Builds the pipeline
    ///
    /// Note: This method creates the pipeline but does not instantiate
    /// the actual sources. Source instantiation requires the specific
    /// source crates to be available and is done by the caller.
    ///
    /// For full source instantiation, use `build_with_factory`.
    pub fn build(self) -> SourceResult<(TelemetryPipeline, Vec<(String, SourceConfig)>)> {
        if self.sources.is_empty() {
            return Err(SourceError::InvalidConfig("no sources configured".into()));
        }

        let pipeline = if let Some(registry) = self.registry {
            TelemetryPipeline::with_registry(self.config, registry)
        } else {
            TelemetryPipeline::new(self.config)
        };

        Ok((pipeline, self.sources))
    }

    /// Builds the pipeline with a source factory
    ///
    /// The factory closure receives each source configuration and
    /// should return a boxed TelemetrySource implementation.
    pub async fn build_with_factory<F, Fut>(self, factory: F) -> SourceResult<TelemetryPipeline>
    where
        F: Fn(String, SourceConfig) -> Fut,
        Fut: std::future::Future<
                Output = SourceResult<Box<dyn pitgun_contract::TelemetrySource + Send>>,
            >,
    {
        if self.sources.is_empty() {
            return Err(SourceError::InvalidConfig("no sources configured".into()));
        }

        let mut pipeline = if let Some(registry) = self.registry {
            TelemetryPipeline::with_registry(self.config, registry)
        } else {
            TelemetryPipeline::new(self.config)
        };

        for (name, config) in self.sources {
            let source = factory(name.clone(), config).await?;
            pipeline.add_source_boxed(&name, source)?;
        }

        Ok(pipeline)
    }
}

/// Preset configurations for common use cases
impl PipelineBuilder {
    /// Creates a builder optimized for high-frequency telemetry (>1kHz)
    pub fn high_frequency() -> Self {
        Self::new()
            .with_channel_capacity(16384)
            .with_merging(Duration::from_micros(500))
    }

    /// Creates a builder optimized for low-latency streaming
    pub fn low_latency() -> Self {
        Self::new().with_channel_capacity(256)
    }

    /// Creates a builder for multi-source aggregation
    pub fn multi_source() -> Self {
        Self::new()
            .with_channel_capacity(8192)
            .with_merging(Duration::from_millis(2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_fluent_api() {
        let builder = PipelineBuilder::new()
            .with_udp("ecu", "0.0.0.0:20777", "ecubridge")
            .with_websocket("ws", "ws://localhost:8080")
            .with_kafka("kafka", &["broker:9092"], "topic", "group")
            .with_mqtt("mqtt", "mqtt://localhost", "telemetry/#", "client")
            .with_physics("physics", "model", 1000.0)
            .with_merging(Duration::from_millis(5));

        assert_eq!(builder.source_count(), 5);
        assert!(builder.config.enable_merging);
    }

    #[test]
    fn builder_presets() {
        let hf = PipelineBuilder::high_frequency();
        assert_eq!(hf.config.channel_capacity, 16384);
        assert!(hf.config.enable_merging);

        let ll = PipelineBuilder::low_latency();
        assert_eq!(ll.config.channel_capacity, 256);

        let ms = PipelineBuilder::multi_source();
        assert!(ms.config.enable_merging);
    }

    #[test]
    fn builder_into_config() {
        let builder = PipelineBuilder::new()
            .with_udp("ecu", "0.0.0.0:20777", "ecubridge")
            .with_channel_capacity(4096);

        let (config, sources) = builder.into_config();

        assert_eq!(config.channel_capacity, 4096);
        assert_eq!(sources.len(), 1);

        match &sources[0].1 {
            SourceConfig::Udp { bind_addr, codec } => {
                assert_eq!(bind_addr, "0.0.0.0:20777");
                assert_eq!(codec, "ecubridge");
            }
            _ => panic!("expected UDP source"),
        }
    }

    #[test]
    fn builder_requires_sources() {
        let result = PipelineBuilder::new().build();
        assert!(result.is_err());
    }
}

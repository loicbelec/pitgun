//! Multi-Source Telemetry Pipeline Example
//!
//! This example demonstrates how to create a telemetry pipeline that
//! aggregates data from multiple sources simultaneously:
//!
//! - UDP: ECUBridge binary format (real hardware)
//! - WebSocket: Real-time game telemetry
//! - Kafka: Streamed historical data
//! - MQTT: IoT sensor data
//!
//! # Running this example
//!
//! ```bash
//! cargo run --example multi_source -- --config config.yaml
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────────────────────────────┐
//! │ UDP :20777  │────▶│                                     │
//! └─────────────┘     │                                     │
//! ┌─────────────┐     │       TelemetryPipeline             │
//! │ WebSocket   │────▶│                                     │────▶ Output
//! └─────────────┘     │   - Frame Merging                   │
//! ┌─────────────┐     │   - Access Control                  │
//! │ Kafka       │────▶│   - Value Conversion                │
//! └─────────────┘     │                                     │
//! ┌─────────────┐     │                                     │
//! │ MQTT        │────▶│                                     │
//! └─────────────┘     └─────────────────────────────────────┘
//! ```

use std::sync::Arc;
use std::time::Duration;

// Note: This example shows the intended API usage.
// Actual imports depend on feature flags and available crates.

/// Configuration for the multi-source pipeline
#[derive(Debug)]
pub struct PipelineConfiguration {
    /// UDP source settings
    pub udp: Option<UdpConfig>,
    /// WebSocket source settings
    pub websocket: Option<WebSocketConfig>,
    /// Kafka source settings
    pub kafka: Option<KafkaConfig>,
    /// MQTT source settings
    pub mqtt: Option<MqttConfig>,
    /// Access control settings
    pub access_control: AccessControlConfig,
}

#[derive(Debug)]
pub struct UdpConfig {
    pub bind_address: String,
    pub codec: String,
    pub multicast_group: Option<String>,
}

#[derive(Debug)]
pub struct WebSocketConfig {
    pub url: String,
    pub reconnect_interval: Duration,
}

#[derive(Debug)]
pub struct KafkaConfig {
    pub brokers: Vec<String>,
    pub topic: String,
    pub group_id: String,
}

#[derive(Debug)]
pub struct MqttConfig {
    pub broker_url: String,
    pub topic_pattern: String,
    pub client_id: String,
    pub qos: u8,
}

#[derive(Debug)]
pub struct AccessControlConfig {
    pub enabled: bool,
    pub default_level: String,
    pub audit_logging: bool,
}

fn main() {
    println!("=== Pitgun Multi-Source Telemetry Pipeline ===\n");

    // Example configuration
    let config = PipelineConfiguration {
        udp: Some(UdpConfig {
            bind_address: "0.0.0.0:20777".into(),
            codec: "ecubridge".into(),
            multicast_group: Some("239.1.1.1".into()),
        }),
        websocket: Some(WebSocketConfig {
            url: "ws://localhost:8080/telemetry".into(),
            reconnect_interval: Duration::from_secs(5),
        }),
        kafka: Some(KafkaConfig {
            brokers: vec!["localhost:9092".into()],
            topic: "telemetry.raw".into(),
            group_id: "pitgun-consumer".into(),
        }),
        mqtt: Some(MqttConfig {
            broker_url: "mqtt://localhost:1883".into(),
            topic_pattern: "sensors/+/telemetry".into(),
            client_id: "pitgun-client".into(),
            qos: 1,
        }),
        access_control: AccessControlConfig {
            enabled: true,
            default_level: "protected".into(),
            audit_logging: true,
        },
    };

    println!("Configuration:");
    println!("  UDP: {:?}", config.udp);
    println!("  WebSocket: {:?}", config.websocket);
    println!("  Kafka: {:?}", config.kafka);
    println!("  MQTT: {:?}", config.mqtt);
    println!("  Access Control: {:?}", config.access_control);
    println!();

    // Demonstrate the intended API usage
    println!("Pipeline Builder API (conceptual):");
    println!();
    println!(r#"
    use pitgun_core::{{PipelineBuilder, PipelineConfig}};
    use pitgun_contract::ParameterRegistry;
    use pitgun_policy::{{AccessController, AccessLevel, Claims}};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::main]
    async fn main() -> Result<(), Box<dyn std::error::Error>> {{
        // Load parameter registry from YAML
        let registry = Arc::new(
            ParameterRegistry::from_yaml("examples/registries/f1_generic.yaml")?
        );

        // Configure access controller
        let mut access = AccessController::with_registry(registry.clone());
        access.set_default_level(AccessLevel::Protected);
        access.set_bulk_access(&[1, 2, 3, 10, 11], AccessLevel::Public);
        access.set_access(100, AccessLevel::Private, Some("engineering".into()));

        // Build the multi-source pipeline
        let (mut pipeline, sources) = PipelineBuilder::new()
            .with_udp("ecu", "0.0.0.0:20777", "ecubridge")
            .with_websocket("game", "ws://localhost:8080/telemetry")
            .with_kafka("stream", &["localhost:9092"], "telemetry.raw", "pitgun")
            .with_mqtt("iot", "mqtt://localhost:1883", "sensors/#", "pitgun")
            .with_merging(Duration::from_millis(2))
            .with_channel_capacity(8192)
            .build()?;

        // Start the pipeline
        pipeline.start().await?;

        // Subscribe to merged output
        let mut rx = pipeline.subscribe();

        // Create user claims for access control
        let user_claims = Claims::new("engineer_01")
            .with_role("engineer")
            .with_team("engineering")
            .with_max_level(AccessLevel::Private);

        // Process frames
        while let Some(frame) = rx.recv().await {{
            // Filter frame based on user access
            let filtered = access.filter_frame(&user_claims, &frame);
            
            println!(
                "Frame: source={{}} seq={{}} samples={{}} (filtered={{}})",
                frame.source_id(),
                frame.sequence(),
                frame.sample_count(),
                filtered.sample_count()
            );

            // Process samples
            for sample in filtered.samples() {{
                if let Some(def) = registry.get(sample.parameter_id) {{
                    println!(
                        "  {{}} = {{:.2}} {{}}",
                        def.name,
                        sample.value,
                        def.unit.as_deref().unwrap_or("")
                    );
                }}
            }}
        }}

        pipeline.stop().await?;
        Ok(())
    }}
"#);

    println!();
    println!("=== Statistics Collection ===");
    println!();
    println!(r#"
    // Get pipeline statistics
    let stats = pipeline.pipeline_stats();
    println!("Frames received: {{}}", stats.get("frames_received").unwrap_or(&0.0));
    println!("Frames output: {{}}", stats.get("frames_output").unwrap_or(&0.0));
    println!("Output rate: {{:.1}} Hz", stats.get("output_rate_hz").unwrap_or(&0.0));

    // Get per-source statistics
    for name in pipeline.source_names() {{
        if let Some(source_stats) = pipeline.source_stats(&name).await {{
            println!("Source {{}}: frames={{}}, errors={{}}", 
                name, source_stats.frames_received, source_stats.errors);
        }}
    }}
"#);

    println!();
    println!("=== Access Control Example ===");
    println!();
    println!(r#"
    use pitgun_policy::{{AccessController, AccessLevel, Claims, InMemoryAuditLog}};
    use std::sync::Arc;

    // Create access controller with audit logging
    let mut access = AccessController::new();
    let audit_log = Arc::new(InMemoryAuditLog::new(1000));
    access.enable_audit(audit_log.clone());

    // Configure parameter access levels
    access.set_access(1, AccessLevel::Public, None);      // Engine RPM - public
    access.set_access(2, AccessLevel::Protected, None);   // Coolant temp - authenticated users
    access.set_access(100, AccessLevel::Private, Some("engineering".into()));  // Team only
    access.set_access(200, AccessLevel::Confidential, None);  // Explicit grant only

    // Check access for different users
    let public_user = Claims::anonymous();
    let authenticated = Claims::new("user1").with_max_level(AccessLevel::Protected);
    let engineer = Claims::new("eng1")
        .with_max_level(AccessLevel::Private)
        .with_team("engineering");

    // Public parameter - accessible to all
    assert!(access.check(&public_user, 1).is_ok());
    assert!(access.check(&authenticated, 1).is_ok());
    assert!(access.check(&engineer, 1).is_ok());

    // Protected parameter - requires authentication
    assert!(access.check(&public_user, 2).is_err());
    assert!(access.check(&authenticated, 2).is_ok());

    // Private parameter - requires team membership
    assert!(access.check(&authenticated, 100).is_err());
    assert!(access.check(&engineer, 100).is_ok());

    // View audit log
    for entry in audit_log.violations() {{
        println!("DENIED: user={{}} param={{}} reason={{:?}}", 
            entry.subject, entry.parameter_id, entry.reason);
    }}
"#);
}

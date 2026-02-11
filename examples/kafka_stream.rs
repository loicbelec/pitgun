//! Kafka Stream Telemetry Example
//!
//! This example demonstrates consuming telemetry data from Apache Kafka,
//! suitable for:
//!
//! - Historical data replay
//! - Multi-consumer architectures
//! - Data pipeline integration
//! - Cloud-native telemetry systems
//!
//! # Prerequisites
//!
//! - Apache Kafka cluster running
//! - Topic created: `telemetry.raw`
//!
//! # Running this example
//!
//! ```bash
//! # Start Kafka (example with Docker)
//! docker-compose up -d kafka
//!
//! # Create topic
//! kafka-topics --create --topic telemetry.raw --bootstrap-server localhost:9092
//!
//! # Run the example
//! cargo run --example kafka_stream
//! ```

use std::time::Duration;

fn main() {
    println!("=== Kafka Stream Telemetry Example ===\n");

    println!("Kafka Source Features:");
    println!("  ✓ Consumer group support for scaling");
    println!("  ✓ Automatic partition assignment");
    println!("  ✓ Offset management (earliest/latest/stored)");
    println!("  ✓ Configurable polling and batching");
    println!();

    println!("Example Usage:");
    println!();
    println!(r#"
    use pitgun_source_kafka::KafkaSource;
    use pitgun_contract::TelemetrySource;

    #[tokio::main]
    async fn main() -> Result<(), Box<dyn std::error::Error>> {{
        // Create Kafka source
        let mut source = KafkaSource::builder()
            .brokers(&["localhost:9092"])
            .topic("telemetry.raw")
            .group_id("pitgun-consumer-group")
            .auto_offset_reset("earliest")  // Start from beginning
            .build()?;
        
        // Start consuming
        source.start().await?;
        
        // Subscribe to frames
        let mut rx = source.subscribe();
        
        println!("Consuming from Kafka topic 'telemetry.raw'...");
        
        while let Some(frame) = rx.recv().await {{
            println!(
                "Kafka frame: partition={{}} offset={{}} seq={{}} samples={{}}",
                frame.metadata().get("partition").unwrap_or(&"?".into()),
                frame.metadata().get("offset").unwrap_or(&"?".into()),
                frame.sequence(),
                frame.sample_count()
            );
            
            // Process telemetry samples
            for sample in frame.samples() {{
                println!(
                    "  [{{:>6}}] param={{:4}} value={{:12.4}}",
                    sample.timestamp_offset,
                    sample.parameter_id,
                    sample.value
                );
            }}
        }}
        
        source.stop().await?;
        Ok(())
    }}
"#);

    println!();
    println!("=== Consumer Groups ===");
    println!();
    println!(r#"
    // Multiple consumers in the same group share partitions
    // for horizontal scaling
    
    // Consumer 1 (Instance A)
    let source_a = KafkaSource::builder()
        .brokers(&["kafka1:9092", "kafka2:9092"])
        .topic("telemetry.raw")
        .group_id("pitgun-processing")  // Same group
        .build()?;
    
    // Consumer 2 (Instance B)
    let source_b = KafkaSource::builder()
        .brokers(&["kafka1:9092", "kafka2:9092"])
        .topic("telemetry.raw")
        .group_id("pitgun-processing")  // Same group
        .build()?;
    
    // Kafka automatically distributes partitions between A and B
"#);

    println!();
    println!("=== Offset Management ===");
    println!();
    println!(r#"
    // Control where to start reading from
    
    // Start from the beginning (replay all data)
    .auto_offset_reset("earliest")
    
    // Start from the end (only new data)
    .auto_offset_reset("latest")
    
    // Start from last committed offset (resume)
    .auto_offset_reset("stored")
    
    // Enable auto-commit for simple use cases
    .enable_auto_commit(true)
    .auto_commit_interval(Duration::from_secs(5))
    
    // Or manual commit for exactly-once semantics
    .enable_auto_commit(false)
    
    // After processing:
    source.commit().await?;
"#);

    println!();
    println!("=== Data Format ===");
    println!();
    println!(r#"
    // Kafka messages can contain different formats
    
    // JSON format (human-readable, larger)
    {{
        "timestamp": 1706400000000000,
        "sequence": 12345,
        "source_id": "car_1",
        "samples": [
            {{"parameter_id": 1, "value": 12500.0, "offset": 0}},
            {{"parameter_id": 2, "value": 95.5, "offset": 100}}
        ]
    }}
    
    // Binary format (compact, efficient)
    // Uses TelemetryFrame serialization with serde
    
    // Choose codec based on producer format
    let source = KafkaSource::builder()
        .with_json_codec()  // or .with_binary_codec()
        .build()?;
"#);

    println!();
    println!("=== Topic Patterns ===");
    println!();
    println!(r#"
    // Subscribe to multiple topics with patterns
    
    let source = KafkaSource::builder()
        .brokers(&["localhost:9092"])
        .topic_pattern("telemetry\\..*")  // Regex pattern
        .group_id("pitgun-all-telemetry")
        .build()?;
    
    // This matches:
    // - telemetry.raw
    // - telemetry.processed
    // - telemetry.car1
"#);

    println!();
    println!("=== Performance Tuning ===");
    println!();
    println!(r#"
    // Optimize for high-throughput scenarios
    
    let source = KafkaSource::builder()
        .brokers(&["kafka:9092"])
        .topic("telemetry.raw")
        .group_id("pitgun-hft")
        // Fetch settings
        .fetch_min_bytes(1024 * 1024)     // 1MB minimum fetch
        .fetch_max_wait_ms(100)            // Max 100ms wait
        .max_partition_fetch_bytes(10 * 1024 * 1024)  // 10MB per partition
        // Batch settings
        .batch_size(1000)                  // Batch up to 1000 messages
        .batch_timeout(Duration::from_millis(50))
        .build()?;
"#);
}

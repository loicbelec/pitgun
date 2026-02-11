//! MQTT IoT Sensor Telemetry Example
//!
//! This example demonstrates receiving telemetry data from IoT sensors
//! via MQTT, suitable for:
//!
//! - Environmental sensors (temperature, pressure, humidity)
//! - Industrial telemetry
//! - Distributed sensor networks
//! - Edge computing scenarios
//!
//! # Prerequisites
//!
//! - MQTT broker running (Mosquitto, HiveMQ, etc.)
//!
//! # Running this example
//!
//! ```bash
//! # Start MQTT broker (example with Docker)
//! docker run -d -p 1883:1883 eclipse-mosquitto
//!
//! # Run the example
//! cargo run --example mqtt_iot
//!
//! # Publish test data
//! mosquitto_pub -t "sensors/temp_01/telemetry" -m '{"value": 23.5}'
//! ```

use std::time::Duration;

fn main() {
    println!("=== MQTT IoT Sensor Telemetry Example ===\n");

    println!("MQTT Source Features:");
    println!("  ✓ QoS levels 0, 1, 2 support");
    println!("  ✓ Wildcard topic subscriptions (+, #)");
    println!("  ✓ Automatic reconnection");
    println!("  ✓ Last Will and Testament (LWT)");
    println!("  ✓ TLS/SSL support");
    println!();

    println!("Example Usage:");
    println!();
    println!(r#"
    use pitgun_source_mqtt::MqttSource;
    use pitgun_contract::TelemetrySource;

    #[tokio::main]
    async fn main() -> Result<(), Box<dyn std::error::Error>> {{
        // Create MQTT source with wildcard subscription
        let mut source = MqttSource::builder()
            .broker("mqtt://localhost:1883")
            .client_id("pitgun-iot-consumer")
            .topic("sensors/+/telemetry")  // + matches single level
            .qos(1)  // At least once delivery
            .build()?;
        
        // Start consuming
        source.start().await?;
        
        // Subscribe to frames
        let mut rx = source.subscribe();
        
        println!("Subscribed to MQTT topic 'sensors/+/telemetry'...");
        
        while let Some(frame) = rx.recv().await {{
            // Extract sensor ID from topic
            let topic = frame.metadata()
                .get("topic")
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            
            println!("MQTT message from '{{}}': {{}} samples", 
                topic, frame.sample_count());
            
            for sample in frame.samples() {{
                println!("  Sensor {{:4}}: {{:.2}}", 
                    sample.parameter_id, sample.value);
            }}
        }}
        
        source.stop().await?;
        Ok(())
    }}
"#);

    println!();
    println!("=== Topic Wildcards ===");
    println!();
    println!(r#"
    // MQTT supports two wildcard characters
    
    // Single-level wildcard (+)
    // Matches exactly one topic level
    .topic("sensors/+/temperature")
    // Matches: sensors/room1/temperature, sensors/room2/temperature
    // Does NOT match: sensors/floor1/room1/temperature
    
    // Multi-level wildcard (#)
    // Matches any number of levels (must be last)
    .topic("sensors/#")
    // Matches: sensors/temp, sensors/room1/temp, sensors/floor1/room1/temp
    
    // Combined example
    .topic("building/+/floor/+/sensors/#")
    // Matches: building/A/floor/3/sensors/temp/room1
"#);

    println!();
    println!("=== QoS Levels ===");
    println!();
    println!(r#"
    // Quality of Service levels for different reliability needs
    
    // QoS 0: At most once (fire and forget)
    // - Fastest, no acknowledgment
    // - Use for non-critical, high-frequency data
    .qos(0)
    
    // QoS 1: At least once (acknowledged delivery)
    // - Message may be delivered multiple times
    // - Good balance of reliability and performance
    .qos(1)
    
    // QoS 2: Exactly once (guaranteed delivery)
    // - Slowest, four-way handshake
    // - Use for critical data where duplicates are problematic
    .qos(2)
"#);

    println!();
    println!("=== Secure Connections ===");
    println!();
    println!(r#"
    // Connect to MQTT broker with TLS
    
    let source = MqttSource::builder()
        .broker("mqtts://secure.mqtt.example.com:8883")
        .client_id("pitgun-secure")
        .topic("sensors/#")
        // Client certificate authentication
        .tls_config(TlsConfig {{
            ca_cert: "/path/to/ca.crt",
            client_cert: Some("/path/to/client.crt"),
            client_key: Some("/path/to/client.key"),
        }})
        // Or username/password authentication
        .credentials("username", "password")
        .build()?;
"#);

    println!();
    println!("=== Last Will and Testament ===");
    println!();
    println!(r#"
    // Configure LWT for connection status monitoring
    
    let source = MqttSource::builder()
        .broker("mqtt://localhost:1883")
        .client_id("pitgun-iot")
        .topic("sensors/#")
        // LWT message sent by broker if connection drops unexpectedly
        .last_will(
            "clients/pitgun-iot/status",  // Topic
            b"offline",                    // Payload
            1,                             // QoS
            true                           // Retain
        )
        .build()?;
"#);

    println!();
    println!("=== IoT Sensor Network Architecture ===");
    println!();
    println!("Typical Topic Structure:");
    println!();
    println!("  site/");
    println!("  ├── building_a/");
    println!("  │   ├── floor_1/");
    println!("  │   │   ├── room_101/");
    println!("  │   │   │   ├── temperature");
    println!("  │   │   │   ├── humidity");
    println!("  │   │   │   └── occupancy");
    println!("  │   │   └── room_102/");
    println!("  │   │       └── ...");
    println!("  │   └── floor_2/");
    println!("  │       └── ...");
    println!("  └── building_b/");
    println!("      └── ...");
    println!();

    println!("=== Payload Formats ===");
    println!();
    println!(r#"
    // Common IoT payload formats
    
    // Simple JSON (most common)
    {{
        "sensor_id": "temp_01",
        "value": 23.5,
        "unit": "celsius",
        "timestamp": 1706400000
    }}
    
    // Compact binary (for constrained devices)
    // [parameter_id: u16][timestamp: u32][value: f32]
    
    // Sparkplug B (industrial IoT standard)
    // Uses protobuf encoding for efficiency
    
    // Configure codec based on payload format
    let source = MqttSource::builder()
        .with_json_codec()
        // or .with_sparkplug_codec()
        // or .with_custom_codec(MyCodec)
        .build()?;
"#);
}

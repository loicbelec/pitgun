//! WebSocket Game Telemetry Example
//!
//! This example demonstrates receiving real-time telemetry data
//! via WebSocket connection from games or simulators.
//!
//! # Use Cases
//!
//! - Racing simulators (iRacing, Assetto Corsa, rFactor)
//! - Flight simulators
//! - Game streaming overlays
//! - Real-time dashboards
//!
//! # Running this example
//!
//! ```bash
//! cargo run --example websocket_game
//! ```

use std::time::Duration;

fn main() {
    println!("=== WebSocket Game Telemetry Example ===\n");

    println!("WebSocket Source Features:");
    println!("  ✓ Automatic reconnection with backoff");
    println!("  ✓ JSON and binary message support");
    println!("  ✓ Ping/pong keepalive");
    println!("  ✓ Connection state monitoring");
    println!();

    println!("Example Usage:");
    println!();
    println!(r#"
    use pitgun_source_ws::AsyncWsSource;
    use pitgun_contract::TelemetrySource;

    #[tokio::main]
    async fn main() -> Result<(), Box<dyn std::error::Error>> {{
        // Create WebSocket source with auto-reconnect
        let mut source = AsyncWsSource::builder("ws://localhost:8080/telemetry")
            .with_reconnect_interval(Duration::from_secs(5))
            .with_max_reconnect_attempts(10)
            .with_ping_interval(Duration::from_secs(30))
            .build()?;
        
        // Start connecting
        source.start().await?;
        
        // Subscribe to frames
        let mut rx = source.subscribe();
        
        println!("Connected to WebSocket server");
        
        while let Some(frame) = rx.recv().await {{
            println!(
                "Game frame: seq={{}} samples={{}}",
                frame.sequence(),
                frame.sample_count()
            );
            
            // Example: Extract common game telemetry values
            for sample in frame.samples() {{
                match sample.parameter_id {{
                    1 => println!("  Speed: {{:.1}} km/h", sample.value),
                    2 => println!("  RPM: {{:.0}}", sample.value),
                    3 => println!("  Gear: {{}}", sample.value as i32),
                    10 => println!("  Throttle: {{:.0}}%", sample.value * 100.0),
                    11 => println!("  Brake: {{:.0}}%", sample.value * 100.0),
                    _ => {{}}
                }}
            }}
        }}
        
        source.stop().await?;
        Ok(())
    }}
"#);

    println!();
    println!("=== Auto-Reconnection ===");
    println!();
    println!(r#"
    // The WebSocket source handles disconnections automatically
    
    let source = AsyncWsSource::builder("ws://localhost:8080/telemetry")
        // Initial delay before first reconnect attempt
        .with_reconnect_interval(Duration::from_secs(1))
        // Maximum delay between attempts (exponential backoff)
        .with_max_reconnect_delay(Duration::from_secs(60))
        // Maximum number of attempts (0 = infinite)
        .with_max_reconnect_attempts(0)
        .build()?;
    
    // Monitor connection state
    loop {{
        let state = source.state().await;
        match state {{
            SourceState::Running => println!("Connected"),
            SourceState::Reconnecting => println!("Reconnecting..."),
            SourceState::Stopped => break,
            _ => {{}}
        }}
        tokio::time::sleep(Duration::from_secs(1)).await;
    }}
"#);

    println!();
    println!("=== Binary vs JSON Messages ===");
    println!();
    println!(r#"
    // The source can handle both binary and JSON formats
    // depending on the codec configuration
    
    use pitgun_codec_udp::{{F1Codec, EcuBridgeCodec}};
    
    // For JSON telemetry (common in game mods)
    let source = AsyncWsSource::builder("ws://localhost:8080/telemetry")
        .with_json_codec()
        .build()?;
    
    // For binary telemetry (more efficient)
    let source = AsyncWsSource::builder("ws://localhost:8080/telemetry")
        .with_codec(F1Codec::new())
        .build()?;
"#);

    println!();
    println!("=== Custom Headers ===");
    println!();
    println!(r#"
    // Add authentication or custom headers to the WebSocket handshake
    
    let source = AsyncWsSource::builder("wss://api.example.com/telemetry")
        .with_header("Authorization", "Bearer <token>")
        .with_header("X-Client-Version", "1.0.0")
        .build()?;
"#);

    println!();
    println!("=== Game-Specific Examples ===");
    println!();
    println!("iRacing:");
    println!("  - WebSocket bridge available via third-party tools");
    println!("  - Typical data: speed, rpm, gear, throttle, brake, g-forces");
    println!();
    println!("Assetto Corsa Competizione:");
    println!("  - Uses shared memory, WebSocket bridge needed");
    println!("  - Similar telemetry format to real F1 data");
    println!();
    println!("rFactor 2:");
    println!("  - Plugin-based WebSocket output");
    println!("  - Configurable data channels");
}

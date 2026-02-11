//! UDP ECUBridge Source Example
//!
//! This example demonstrates receiving telemetry data via UDP
//! using the ECUBridge binary protocol format.
//!
//! # ECUBridge Protocol
//!
//! ECUBridge uses a custom 8193-byte UDP packet format:
//! - Header (32 bytes): Magic, version, sequence, timestamp, sample count
//! - Samples (N * 16 bytes): Parameter ID, timestamp offset, value, quality
//! - Footer (8 bytes): CRC32, end marker
//!
//! # Running this example
//!
//! ```bash
//! # Start the example (listens on UDP port 20777)
//! cargo run --example udp_ecubridge
//!
//! # In another terminal, send test data
//! # (or connect real ECUBridge hardware)
//! ```
//!
//! # Multicast Support
//!
//! For multicast reception (typical in pit lane setups):
//! ```bash
//! cargo run --example udp_ecubridge -- --multicast 239.1.1.1
//! ```

use std::time::Duration;

fn main() {
    println!("=== UDP ECUBridge Source Example ===\n");

    println!("ECUBridge Packet Format (8193 bytes):");
    println!("┌────────────────────────────────────────┐");
    println!("│ Header (32 bytes)                      │");
    println!("│   Magic: 0x45435542 ('ECUB')           │");
    println!("│   Version: u16                         │");
    println!("│   Sequence: u32                        │");
    println!("│   Timestamp: u64 (microseconds)        │");
    println!("│   Sample Count: u16                    │");
    println!("│   Flags: u16                           │");
    println!("│   Reserved: 8 bytes                    │");
    println!("├────────────────────────────────────────┤");
    println!("│ Samples (N × 16 bytes each)            │");
    println!("│   Parameter ID: u32                    │");
    println!("│   Timestamp Offset: u32 (microseconds) │");
    println!("│   Value: f64                           │");
    println!("├────────────────────────────────────────┤");
    println!("│ Footer (8 bytes)                       │");
    println!("│   CRC32: u32                           │");
    println!("│   End Marker: 0x454E4421 ('END!')      │");
    println!("└────────────────────────────────────────┘");
    println!();

    println!("Example Usage:");
    println!();
    println!(r#"
    use pitgun_source_udp::AsyncUdpSource;
    use pitgun_codec_udp::EcuBridgeCodec;
    use pitgun_contract::TelemetrySource;
    use std::net::SocketAddr;

    #[tokio::main]
    async fn main() -> Result<(), Box<dyn std::error::Error>> {{
        // Create UDP source with ECUBridge codec
        let bind_addr: SocketAddr = "0.0.0.0:20777".parse()?;
        let codec = EcuBridgeCodec::new();
        
        let mut source = AsyncUdpSource::new(bind_addr, codec);
        
        // Optional: Enable multicast for pit lane reception
        // source.join_multicast("239.1.1.1".parse()?)?;
        
        // Start receiving
        source.start().await?;
        
        // Subscribe to frames
        let mut rx = source.subscribe();
        
        println!("Listening on UDP port 20777...");
        println!("Waiting for ECUBridge packets...");
        
        while let Some(frame) = rx.recv().await {{
            println!(
                "Received frame: seq={{}} samples={{}} ts={{}}",
                frame.sequence(),
                frame.sample_count(),
                frame.timestamp()
            );
            
            // Process samples
            for sample in frame.samples() {{
                println!(
                    "  Param {{:4}}: {{:12.4}} @ +{{}}µs",
                    sample.parameter_id,
                    sample.value,
                    sample.timestamp_offset
                );
            }}
            
            // Check for packet loss
            let stats = source.stats().await;
            if stats.packets_lost > 0 {{
                println!("WARNING: Packet loss detected ({{}}/{{}})", 
                    stats.packets_lost, stats.packets_received);
            }}
        }}
        
        source.stop().await?;
        Ok(())
    }}
"#);

    println!();
    println!("=== Multicast Configuration ===");
    println!();
    println!(r#"
    // Multicast is commonly used when multiple receivers need the same data
    // (e.g., timing screens, strategy systems, telemetry displays)
    
    let mut source = AsyncUdpSource::new(bind_addr, codec);
    
    // Join multicast group
    source.join_multicast("239.1.1.1".parse()?)?;
    
    // Optional: Set multicast TTL for multi-hop networks
    source.set_multicast_ttl(2)?;
    
    // Optional: Allow receiving from any interface
    source.set_multicast_any_interface(true)?;
"#);

    println!();
    println!("=== Sequence Tracking ===");
    println!();
    println!(r#"
    // The AsyncUdpSource includes automatic sequence tracking
    // to detect packet loss and out-of-order delivery
    
    let stats = source.stats().await;
    
    println!("Packet Statistics:");
    println!("  Received: {{}}", stats.packets_received);
    println!("  Lost: {{}}", stats.packets_lost);
    println!("  Out of order: {{}}", stats.packets_out_of_order);
    println!("  Duplicates: {{}}", stats.packets_duplicate);
    println!("  Loss rate: {{:.2}}%", 
        stats.packets_lost as f64 / stats.packets_received as f64 * 100.0);
"#);

    println!();
    println!("=== Error Handling ===");
    println!();
    println!(r#"
    // Handle common UDP errors gracefully
    
    match source.start().await {{
        Ok(_) => println!("Source started"),
        Err(e) if e.is_bind_error() => {{
            eprintln!("Failed to bind to port (already in use?)");
        }}
        Err(e) if e.is_network_error() => {{
            eprintln!("Network error: {{}}", e);
        }}
        Err(e) => return Err(e.into()),
    }}
"#);
}

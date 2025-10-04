# Pitgun Dev Journal 🏎️ 

## Introduction

**Pitgun** is my personal journey into building a modular telemetry and data processing framework in Rust. 

The project explores how to ingest, emulate, and analyze high-frequency data streams - similar to those used in Formula 1 telemetry systems - while applying modern Rust concepts and patterns.

### 🎯 Goals  
- Learn and apply modern Rust in a real-world, performance-critical context  
- Build a modular, low-latency data pipeline  
- Experiment with UDP streaming, parallel ingestion, and high-frequency emulation  
- Bridge **Formula 1 telemetry** with **High-Frequency Trading (HFT)** paradigms - both domains where *latency and precision decide winners*  

This repository is a **learning log**. I’m documenting not just the code, but the thought process, mistakes, and lessons along the way.  

By combining insights from **Formula 1 telemetry** and **High-Frequency Trading**, Pitgun is my sandbox to experiment with ultra-low-latency data systems.

## Table of Contents
- [Introduction](#introduction)
- [Project Structure](#project-structure)
- [Roadmap](#roadmap)
- [Step 1 - First Emulator](#step-1---first-emulator)
- Step 2 - Parallel Processing (WIP)

## Project structure
Pitgun is organized as a Rust workspace composed of several crates:

| Crate | Purpose |
|-------|----------|
| `pitgun-core` | Core library: data structures, parsing, pipeline operators |
| `pitgun-cli` | Command-line interface: ingest, transform, export |
| `pitgun-emulator` | UDP emitter: replays CSV datasets at configurable pace |

## Roadmap
- [x] Create Rust workspace with `core`, `cli`, `emulator`  
- [x] Implement UDP emission from CSV datasets  
- [ ] Add sequence numbers and loss detection  
- [ ] Implement a `pitgun-listener` crate for packet decoding  
- [ ] Explore sinks: Parquet, Kafka, Arrow  
- [ ] Add benchmarks and performance profiling  
- [ ] Study parallels with HFT market data (UDP multicast, order books, latency profiling)  
- [ ] Publish crates on [crates.io](https://crates.io) when stable 

## Step 1 - First Emulator

### Context

In **Formula 1**, telemetry is both a technological backbone and a closely guarded secret. Every team uses the [Atlas Ecosystem](https://www.motionapplied.com/products/ATLAS), developed by *Motion Applied* (formerly *McLaren Applied*), which provides a complete data acquisition toolchain - from the ECU (Electronic Control Unit) in the car to the dashboard software you see lighting up in the pitlane.

Telemetry is split into several channels. One stream is sent directly to the FIA, which monitors a subset of live telemetry data in real time to enforce sporting and technical regulations. These streams travel through the paddock network using **UDP multicast**, allowing broadcast to multiple recipients - but each flow is **encrypted**, ensuring teams cannot read each other’s data.

### Objective

Reproduce a minimalistic version of this system - a first step toward a modular telemetry framework capable of emulating real F1 data flow with synthetic data.

### Implementation

The first channel emulated is the engine speed, known under the Atlas namespace as `FIA:nEngine`.

**Design goals:**
- **Data source:** simple CSV time series.
- **Transport:** **UDP multicast** to mimic trackside broadcast patterns.
- **Encryption:** lightweight XOR-style scrambling (placeholder for proprietary ciphers).
- **Replay pacing:** optional pacing to preserve timing between samples.

**Example dataset:**
```csv
Timestamp,Value
62076104000000,1234.5
62076105000000,1235.2
```

**Conventions & CLI:**
- Channel name is inferred from the filename, e.g. `FIA-nEngine.csv`.
- Each row is replayed over UDP; by default as fast as possible, or **paced** with `--pace` to respect inter-sample deltas.
- Flags (draft):
  - `--file <path>`: CSV file to replay
  - `--multicast <addr:port>`: UDP multicast target
  - `--pace`: enable pacing using CSV timestamps
  - `--key <hex>`: enable simple XOR encryption with provided key
  - `--loop`: continuous loop over the dataset

```text
pitgun-emulator \
  --target 239.10.0.1:5001 \
  --csv ./FIA-nEngine.csv \
  --pace \
  --channel FIA:nEngine \
  --mcast_ttl 1
```

### Code Highlights
#### Channel name inference
Derive the channel from the filename, unless the user overrides it with --channel.
```rust
// Channel = --channel or file stem (e.g., "FIA-nEngine.csv" -> "FIA-nEngine")
let channel = args
    .channel
    .clone()
    .or_else(|| args.csv.file_stem().map(|s| s.to_string_lossy().to_string()))
    .unwrap_or_else(|| "unknown-channel".to_string());
```
#### Pacing loop (relative time)
Turn a CSV into a live stream by respecting timestamp deltas.
```rust
let start_monotonic = Instant::now();
let t0_ns = rows.first().unwrap().ts_ns;

for (i, r) in rows.iter().enumerate() {
    if args.pace {
        let target_elapsed_ns = r.ts_ns.saturating_sub(t0_ns);
        let target_elapsed = Duration::from_nanos((target_elapsed_ns as u64).min(u64::MAX));
        let now_elapsed = start_monotonic.elapsed();
        if target_elapsed > now_elapsed {
            sleep(target_elapsed - now_elapsed);
        }
    }
    // send frame...
}
```

#### Timestamp parsing heuristics
Accept seconds / ms / µs / ns (numeric) and RFC3339/ISO (string), normalize to ns.
```rust
fn parse_timestamp_to_ns(s: &str) -> Result<u128> {
    // numeric? -> scale by magnitude
    if let Ok(x) = s.parse::<f64>() {
        let abs = x.abs();
        return Ok(if abs < 1e6      { (x * 1e9) as u128 }   // seconds
                 else if abs < 1e12 { (x * 1e6) as u128 }   // ms
                 else if abs < 1e15 { (x * 1e3) as u128 }   // µs
                 else               {  x as u128 });         // ns
    }
    // RFC3339
    if let Ok(t) = time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339) {
        return Ok(t.unix_timestamp_nanos() as u128);
    }
    // Loose ISO "YYYY-MM-DD HH:MM:SS.sss"
    if let Ok(fmt) = time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]") {
        if let Ok(t) = time::PrimitiveDateTime::parse(s, &fmt) {
            return Ok(t.assume_utc().unix_timestamp_nanos() as u128);
        }
    }
    anyhow::bail!("unsupported timestamp format: {}", s);
}
```

#### Minimal wire framing
A compact, evolvable on-the-wire representation.
```rust
/// [len_channel:u16][channel][ts_csv:u128 LE][value:f64 LE]
fn encode_frame(channel: &str, ts_csv_ns: u128, value: f64) -> Vec<u8> {
    let name = channel.as_bytes();
    let mut buf = Vec::with_capacity(2 + name.len() + 16 + 8);
    let len = u16::try_from(name.len()).unwrap_or(u16::MAX);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(name);
    buf.extend_from_slice(&ts_csv_ns.to_le_bytes());
    buf.extend_from_slice(&value.to_le_bytes());
    buf
}
```

#### Multicast-aware socket
Enable multicast TTL automatically when the target is in 224.0.0.0/4.
```rust
fn make_udp_socket(target: &SocketAddr, mcast_ttl: u32) -> Result<Socket> {
    let domain = match target {
        SocketAddr::V4(_) => Domain::IPV4,
        SocketAddr::V6(_) => Domain::IPV6,
    };
    let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

    if let SocketAddr::V4(addr_v4) = target {
        let first_octet = addr_v4.ip().octets()[0];
        if (224..=239).contains(&first_octet) {
            sock.set_multicast_ttl_v4(mcast_ttl)?;
        }
    }
    Ok(sock)
}
```

### Architecture Notes

- **Layered design:** ingestion (CSV) → processing (pacing, framing, crypto) → transport (UDP).
- **Channel abstraction:** each source file maps to a telemetry channel (e.g., `FIA:nEngine`, `Arbitrator-rThrottlePedal`).
- **Network realism:** multicast group join, packet sizing, and low-latency send path.
- **Security stub:** pluggable crypto module so the XOR can be swapped for stronger schemes later.

### Learnings

- A static CSV becomes a live stream once you respect timing and framing.
- Multicast + lightweight encryption gives a realistic trackside feel without overengineering.
- Clear separation of concerns makes it easy to:
  - Add parallel channels,
  - Swap encryption,
  - Change transport (e.g., QUIC/UDP, NATS) without touching business logic.


### What’s Next (Bridge to Step 2)

- Expand to multi-channel replay (engine speed, throttle, gear) with parallel workers.
- Introduce session metadata (car, stint, lap) and timebase alignment across channels.
- Add a receiver tool to validate packet loss, latency, and decryption correctness.
- Prepare a binary packet format (header + payload) for versioning and backward compatibility.
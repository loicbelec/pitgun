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
- [Step 2 - Parallel Processing (WIP)](#step-2---parallel-processing)

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

The first channel emulated is the engine speed, known under the Atlas namespace as `FIA-nEngine`.

**Design goals:**
- **Data source:** simple CSV time series.
- **Transport:** **UDP multicast** to mimic trackside broadcast patterns.
- **Encryption:** lightweight XOR-style scrambling (placeholder for proprietary ciphers).
- **Replay pacing:** optional pacing to preserve timing between samples.

**Example dataset:**
```csv
Timestamp,ChannelValue
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
  --target 127.0.0.1:5001 \
  --csv datasets/telemetry/FIA-nEngine.csv \
  --pace
```

### Minimal wire framing
Each telemetry frame sent by the emulator follows a compact binary layout:

```mermaid
flowchart LR
    FRAME["📦 Frame"]
    FRAME --> LEN["len_channel : u16"]
    FRAME --> CH["channel : [u8]"]
    FRAME --> TS["ts_csv_ns : u128 (LE)"]
    FRAME --> VAL["value : f64 (LE)"]
````

| Field | Type | Size (bytes) | Endianness | Description |
|-------|------|--------------|-------------|--------------|
| `len_channel` | `u16` | 2 | Little-Endian | Length of the channel name in bytes |
| `channel` | UTF-8 string | variable | — | Channel name, e.g. `"FIA-nEngine"` |
| `ts_csv_ns` | `u128` | 16 | Little-Endian | Timestamp from the CSV (in nanoseconds) |
| `value` | `f64` | 8 | Little-Endian | Channel value |

#### 🧠 Example

For a frame where:
- `channel = "FIA-nEngine"`
- `ts_csv_ns = 62076104000000`
- `value = 1234.5`

the serialized bytes look like this:

```
╔════════════════════════════════════════════════════════════════════════╗
║  Field             │ Bytes (hex)                                       ║
╟────────────────────┼───────────────────────────────────────────────────╢
║ len_channel (11)   │ 0B 00                                             ║
║ channel ("FIA-nEngine") │ 46 49 41 3A 6E 45 6E 67 69 6E 65             ║
║ ts_csv_ns          │ 00 C0 5F 73 63 00 00 00 00 00 00 00 00 00 00 00   ║
║ value (1234.5)     │ 00 00 00 00 00 49 93 40                           ║
╚════════════════════════════════════════════════════════════════════════╝
```

The code is as following.
```rust
/// [len_channel:u16][channel][ts_csv_ns:u128 LE][value:f64 LE]
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

### Architecture Notes

- **Layered design:** ingestion (CSV) → processing (pacing, framing, crypto) → transport (UDP).
- **Channel abstraction:** each source file maps to a telemetry channel (e.g., `FIA-nEngine`, `Arbitrator-rThrottlePedal`).
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

## Step 2 - Parallel Processing
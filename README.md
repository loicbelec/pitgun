[![Pitgun](docs/img/pitgun_transparent.png)](https://pitgun.loicbelec.com)

## What is Pitgun?
Pitgun is a modular Rust workspace for telemetry and high-frequency data processing. It ingests raw signals from multiple sources, applies manifest-driven processing, and emits channels and metrics in real time.

## ⚠️ WARNING
This repository is **under active development**. Interfaces may change.

## 🧱 Crates

### Contract & Core
| Crate | Description |
|-------|-------------|
| **pitgun-contract** | `TelemetrySource` trait, `TelemetryFrame` model, `ParameterRegistry` |
| **pitgun-core** | Formula engine, multi-source pipeline, converter service, manifests |
| **pitgun-policy** | Access control, rate limiting, JWT verification |
| **pitgun-signing** | Cryptographic signing utilities |

### Codecs
| Crate | Description |
|-------|-------------|
| **pitgun-codec-udp** | UDP binary wire format decoding |
| **pitgun-codec-json** | SessionEnvelope JSON codec |

### Sources
All sources implement the `TelemetrySource` trait from `pitgun-contract`.

| Crate | Transport | Use Case |
|-------|-----------|----------|
| **pitgun-source-udp** | UDP unicast/multicast | Binary telemetry, sensors |
| **pitgun-source-ws** | WebSocket | Games, web apps, JSON streams |
| **pitgun-source-kafka** | Kafka | High-volume data platforms |
| **pitgun-source-mqtt** | MQTT | IoT devices, pub/sub |
| **pitgun-source-physics** | In-process | Simulated/computed channels |

### Optional
| Crate | Description |
|-------|-------------|
| **pitgun-emulator** | Dataset playback and synthetic channels |

## 🧰 Apps
- **pitgun-cli**: Command-line interface to ingest, transform, and export telemetry data

## ⚙️ Features

### Multi-Source Pipeline
- Ingest from UDP, WebSocket, Kafka, MQTT simultaneously
- Unified `TelemetryFrame` format across all sources
- Parameter registry with YAML definitions

### Processors
- `channel_filter` - whitelist channels
- `scale` - multiply channel by a factor
- `segment_aggregate` - window by segment key (mean/max/min/stddev/count/sum)
- `stats` - per-channel counts and gaps

### Sinks
- Console JSON printer
- Per-channel CSV recording

### Wire Formats
- **UDP v1**: Binary format `[len_channel:u16][channel][ts_ns:u128 LE][value:f64 LE]`
- **JSON**: SessionEnvelope with schema versioning

## 🚀 Quickstart

**1) Emit telemetry from CSV:**
```bash
cargo run -p pitgun-emulator -- \
  --target 127.0.0.1:5001 \
  --input nEngine=datasets/telemetry/nEngine.csv \
  --input throttle=datasets/telemetry/rThrottle.csv \
  --pace
```

**2) Subscribe with a manifest-driven pipeline:**
```bash
cargo run -p pitgun-cli -- subscribe --config manifests/dummy-pitgun.yaml
```

## 📚 Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Core architecture and crate layout
- [docs/WIRE_FORMATS.md](docs/WIRE_FORMATS.md) - Wire protocol specifications
- [docs/segment_aggregation.md](docs/segment_aggregation.md) - Window aggregation feature

## 🧭 Roadmap

- **Event reliability**: Sequence numbers, loss detection
- **Typed wire format**: Unified serialization across all components
- **Ecosystem**: Parquet sink, Arrow integration
- **Performance**: Benchmarks, memory profiling, throughput optimization

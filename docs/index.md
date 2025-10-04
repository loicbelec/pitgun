# Pitgun 🦀

**Pitgun** is my personal journey into building a modular telemetry and data processing framework in Rust.  
The goal: explore how to ingest, emulate, and analyze high-frequency data streams (like F1 telemetry) while learning modern Rust patterns.

I also want to **bridge the Formula 1 world with High-Frequency Trading (HFT)** — two domains where **latency, data throughput, and precision** make the difference between winning and losing.  

## 🏎️ Why Pitgun?
- Passion for Formula 1 and real-time data systems
- Desire to get hands-on with **Rust** in a real-world project
- Experiment with **UDP streaming, data pipelines, and high-frequency emulation**
- Draw parallels between **F1 telemetry pipelines** and **HFT market data feeds**

## 🧩 Project structure
Pitgun is a Rust workspace with multiple crates:

- **`pitgun-core`** → core library: data structures, parsing, pipeline operators  
- **`pitgun-cli`** → command-line interface for ingest, transform, and export  
- **`pitgun-emulator`** → UDP emitter that replays CSV datasets at configurable pace  

---

## 📂 Example dataset
The first dataset I used is a simple CSV:

```csv
Timestamp,ChannelValue
62076104000000,1234.5
62076105000000,1235.2
```

- Channel name is inferred from the filename, e.g. `FIA-nEngine.csv`.  
- Each row is replayed over UDP with optional pacing (`--pace` flag).  

## 🔧 Usage
Run the emulator with pacing at 1 kHz:

```bash
cargo run -p pitgun-emulator -- \
  --target 127.0.0.1:5001 \
  --csv datasets/telemetry/FIA-nEngine.csv \
  --pace
```

And listen on the port with:
```bash
nc -klu 5001
```

## 🛣️ Roadmap
- [x] Create Rust workspace with `core`, `cli`, `emulator`
- [x] Implement UDP emission from CSV datasets
- [ ] Add sequence numbers and loss detection in frames
- [ ] Implement a `pitgun-listener` crate to decode packets
- [ ] Explore sinks: Parquet, Kafka, Arrow
- [ ] Add benchmarks and performance profiling
- [ ] Study parallels with HFT market data (UDP multicast, order books, latency profiling)
- [ ] Publish crates on crates.io when stable

## ✍️ Notes
This repository is also a **learning log**. I’m documenting not just the code, but the thought process, mistakes, and lessons along the way.  

By combining insights from **Formula 1 telemetry** and **High-Frequency Trading**, Pitgun is my sandbox to experiment with ultra-low-latency data systems.
# Pitgun 🦀

**Pitgun** is my personal journey into building a modular telemetry and data processing framework in Rust.  
The goal: explore how to ingest, emulate, and analyze high-frequency data streams (like F1 telemetry) while learning modern Rust patterns.

I also want to **bridge the Formula 1 world with High-Frequency Trading (HFT)** — two domains where **latency, data throughput, and precision** make the difference between winning and losing.  

---

## 🚀 Why Pitgun?
- Passion for Formula 1 and real-time data systems
- Desire to get hands-on with **Rust** in a real-world project
- Experiment with **UDP streaming, data pipelines, and high-frequency emulation**
- Draw parallels between **F1 telemetry pipelines** and **HFT market data feeds**

---

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
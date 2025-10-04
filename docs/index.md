# Pitgun 🏎️ 

## Table of Contents
- [Introduction](#introduction)
- [Project Structure](#project-structure)
- [Roadmap](#roadmap)
- [Notes](#notes)
- [Step 1 - First Emulator](./first-emulator.md)
- Step 2 - Parallel Processing (WIP)

## Introduction

**Pitgun** is my personal journey into building a **modular telemetry and data processing framework in Rust**.  
The project explores how to ingest, emulate, and analyze **high-frequency data streams** - similar to those used in **Formula 1 telemetry systems** - while applying modern Rust concepts and patterns.

### 🎯 Goals  
- Learn and apply **modern Rust** in a real-world, performance-critical context  
- Build a **modular, low-latency data pipeline**  
- Experiment with **UDP streaming, parallel ingestion, and high-frequency emulation**  
- Bridge **Formula 1 telemetry** with **High-Frequency Trading (HFT)** paradigms - both domains where *latency and precision decide winners*  


## Project structure
Pitgun is organized as a **Rust workspace** composed of several crates:

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

## Notes
This repository is a **learning log**. I’m documenting not just the code, but the thought process, mistakes, and lessons along the way.  

By combining insights from **Formula 1 telemetry** and **High-Frequency Trading**, Pitgun is my sandbox to experiment with ultra-low-latency data systems.
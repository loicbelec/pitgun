# Step 1 - First Emulator

## Context

In **Formula 1**, telemetry is both a technological backbone and a closely guarded secret. Every team uses the **[Atlas Ecosystem](https://www.motionapplied.com/products/ATLAS)**, developed by *Motion Applied* (formerly *McLaren Applied*), which provides a complete data acquisition toolchain - from the **ECU** (Electronic Control Unit) in the car to the dashboard software you see lighting up in the pitlane.

Telemetry is split into several channels. One stream is sent directly to the FIA, which monitors a subset of live telemetry data in real time to enforce sporting and technical regulations. These streams travel through the paddock network using **UDP multicast**, allowing broadcast to multiple recipients - but each flow is **encrypted**, ensuring teams cannot read each other’s data.

## Objective

Reproduce a **minimalistic** version of this system - a first step toward a modular telemetry framework capable of emulating real F1 data flow with **synthetic data**.

## Implementation

The first **channel** emulated is the **engine speed**, known under the Atlas namespace as `FIA:nEngine`.

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

## Architecture Notes

- **Layered design:** ingestion (CSV) → processing (pacing, framing, crypto) → transport (UDP).
- **Channel abstraction:** each source file maps to a telemetry channel (e.g., `FIA:nEngine`, `Arbitrator-rThrottlePedal`).
- **Network realism:** multicast group join, packet sizing, and low-latency send path.
- **Security stub:** pluggable crypto module so the XOR can be swapped for stronger schemes later.

## Learnings

- A static CSV becomes a **live stream** once you respect timing and framing.
- Multicast + lightweight encryption gives a realistic trackside feel without overengineering.
- Clear separation of concerns makes it easy to:
  - Add **parallel channels**,
  - Swap **encryption**,
  - Change **transport** (e.g., QUIC/UDP, NATS) without touching business logic.


## What’s Next (Bridge to Step 2)

- Expand to **multi-channel** replay (engine speed, throttle, gear) with **parallel workers**.
- Introduce **session metadata** (car, stint, lap) and **timebase alignment** across channels.
- Add a **receiver** tool to validate packet loss, latency, and decryption correctness.
- Prepare a **binary packet** format (header + payload) for versioning and backward compatibility.
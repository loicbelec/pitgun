## 1. Overview

Pitgun is a modular telemetry and data processing framework inspired by Formula 1 telemetry stacks and High-Frequency Trading infrastructure. The project’s goal is to ingest, emulate, and analyze high-frequency signals with Rust primitives while learning how real motorsport systems multiplex thousands of channels. The workspace is organized into `pitgun-core` for common data structures and runtime traits, `pitgun-cli` for orchestration and observation, and `pitgun-emulator` for replaying CSV datasets as UDP telemetry. Together they reproduce a realistic flow: CSV samples are merged, framed into the Pitgun UDP packet (`[len:u16][channel][ts:u128][value:f64]`), transported over unicast or multicast, decoded into reusable events, processed for monitoring, and finally persisted or visualized.

## 2. Core Concepts

The architecture revolves around reproducible telemetry timelines, unified channel semantics, and a layered transport/process/persist pipeline. `pitgun-core` defines the `Event`/`EventBatch` model plus the `Source`, `Processor`, and `Sink` traits so every crate exchanges the same structures. `pitgun-cli` acts as the user-facing orchestrator: it spins up a transport-specific source adapter, chains processors for filtering or statistics, and pushes batches into sinks such as JSON printing or CSV export. `pitgun-emulator` feeds the system by reading CSV time series (e.g., `FIA-nEngine.csv`, `Arbitrator-rThrottlePedal.csv`), pacing them when required, and emitting packets over UDP multicast or unicast. The design mirrors real ECU stacks: ingestion, processing, and emission are isolated so transports (UDP, gRPC, Kafka) and downstream consumers (CSV, Parquet, diagnostics) can be expanded independently.

## 3. Event Model

Channels are modeled as functions of time $C_i(t)$ and predicates $ \varphi $ evaluate boolean conditions such as “engine speed > 10,000 rpm” or “throttle > 0.8”. An elementary event is the indicator function $E_\varphi(t)=\mathbf{1}\{\varphi(\mathbf{C}(t))\}$ whose rising and falling edges delimit active intervals $[t_i^{\uparrow}, t_i^{\downarrow})$. From these intervals we derive duration ($T_\varphi$), occurrence count ($N_\varphi$), and duty ratio ($\rho_\varphi$). Composite events follow logical operators: $E_{\varphi_1 \land \varphi_2}=E_{\varphi_1}\cdot E_{\varphi_2}$, $E_{\varphi_1 \lor \varphi_2}=\max(E_{\varphi_1},E_{\varphi_2})$, and $E_{\neg \varphi_1}=1-E_{\varphi_1}$. In practice, Pitgun uses these definitions to build temporal masks that isolate high-load phases (e.g., simultaneous high `nEngine` and `rThrottlePedal`). Analysts can gate raw telemetry directly (no interpolation) or resample/interpolate to align channels on a shared grid—each approach trades timestamp fidelity for continuity. `Event` captures each timestamped measurement (channel, `ts_ns`, numeric `value`), while `EventBatch` groups contiguous samples that flow through the pipeline.

## 4. Pipeline Model

Every Pitgun runtime follows the same pipeline: a transport-facing `Source` produces `EventBatch` instances, ordered processors mutate or annotate the batch, and a `Sink` consumes the result. `pitgun-cli` builds this pipeline dynamically based on CLI flags. At runtime it selects a transport adapter (`pitgun-source-udp` today), hands batches through processors (channel gating, statistics, event masking), and finally fans out to sinks for visualization, monitoring, or storage. This layered approach ensures that `pitgun-core` remains the shared semantic layer while transports and outputs evolve. The CLI’s monitoring responsibilities—reporting throughput, packet rate, channel counts, gap detection—are implemented as processors so they can run regardless of source or sink. 

## 5. Source

`Source` abstracts telemetry providers. `pitgun-emulator` supplies data by reading CSV files, running a k-way merge across channels, optionally pacing using timestamp deltas, and encoding each record into the Pitgun UDP frame. Networking supports both unicast (`127.0.0.1:5001`) and multicast (`239.10.0.1:5001`) with correct TTL and loopback settings, mirroring motorsport broadcast patterns. On the receiving side, `pitgun-cli` instantiates `pitgun-source-udp`: it binds to the requested interface, joins multicast groups, parses `[len:u16][channel][ts:u128][value:f64]` frames, applies optional channel filters, and batches events according to `batch_max_len`/`batch_max_ns`. Each source remains transport-specific, but the trait boundary guarantees that downstream logic only sees normalized `EventBatch` instances.

## 6. Processor

Processors operate on mutable batches between ingestion and output. They implement filtering (retaining a subset of channels), compute composite events using the mathematical model above, or derive diagnostics such as per-channel counters, throughput, and gap tracking. The statistics module displayed by `pitgun-cli`—`frames=28054 rate=449.5 fps gaps=0 chans=2 nEngine:25503 throttle:2551`—is implemented as a processor so it can sit alongside other transformations like event masking or interpolation. Processors can also manage pacing logic, gating, and future features such as interpolation or energy-balance calculations since the theoretical groundwork for predicates and intervals already exists in the journal.

## 7. Sink

`Sink` consumers turn processed batches into user-facing artifacts. `pitgun-cli` already exposes a JSON-like console sink for human inspection and a CSV sink that records each channel to `Timestamp,ChannelValue` files. The CLI is designed to expand toward Parquet, Arrow, or Kafka sinks; this capability is part of its “Recording and Replay Support” responsibilities. Because sinks only receive normalized events, they can focus solely on formatting, durability, or transport-specific guarantees (e.g., flushing records, pushing to a metrics database) while processors handle heavy computation upstream.

## 8. Existing Implementations

- `pitgun-emulator` (UDP emitter): Reads datasets such as `FIA-nEngine.csv` and `Controller-rThrottleR.csv`, derives channel names from filenames, supports XOR-style scrambling, and replays multiple channels concurrently via the k-way merge and multicast/unicast networking modes.
- `pitgun-cli` (orchestrator): Provides the `subscribe` command that binds to UDP, joins multicast groups, decodes frames, applies processors (channel filters, stats), and fans out to sinks (JSON console, CSV recorder). Its monitoring mode continuously prints throughput diagnostics and per-channel counters, acting as a real-time health probe.
- `pitgun-core` (shared layer): Defines `Event`, `EventBatch`, `Source`, `Processor`, `Sink`, and the overall pipeline contract so transports and outputs remain interchangeable. It also carries higher-level concepts such as `SessionMeta` and `Quality` when session context is required.

Sample CLI output illustrates the combined effect:

```text
{"channel":"throttle","ts_ns":37325891000000,"value":0.64}
{"channel":"nEngine","ts_ns":37325892000000,"value":0}
...
frames=27941 rate=448.4 fps gaps=0 chans=2  nEngine:25401  throttle:2540
```

## 9. Future Extensions

The roadmap includes adding more sources (gRPC, Kafka), expanding sinks (Parquet, Arrow, Kafka), and interfacing with Python via FFI. Benchmarking, performance profiling, and deeper studies of HFT-style multicast behavior are slated to keep latency visible. `pitgun-cli` will gain additional transports (e.g., `pitgun-source-grpc`, `pitgun-source-kafka`) while keeping the same processor/sink surfaces. The emulator will evolve toward multi-channel replay with parallel workers, latency monitoring, and richer session metadata (car, stint, lap). Once the architecture stabilizes, the crates will be published to crates.io, turning this learning log into a reusable telemetry toolkit.

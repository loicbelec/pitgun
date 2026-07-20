# Pitgun Architecture

Pitgun is a Rust framework for executing, observing, replaying, and verifying
deterministic time-series simulations.

The racing game is the first production domain built on top of the framework. It
generates telemetry and exercises the distributed simulation flow, but racing
concepts are not the framework boundary. Generic crates should be able to carry
and validate racing payloads without understanding vehicle, circuit, tire, lap,
or race semantics.

This document describes the **core architecture** of the Pitgun framework, its
**key concepts**, and how the workspace crates collaborate.

## 1. Goals & Non-Goals

### 1.1 Goals

Pitgun is designed to:

- Execute versioned domain workloads with stable randomness and explicit
  logical ordering.
- Emit typed telemetry and canonical evidence from each run.
- Persist portable Run Bundles that can be replayed and verified independently.
- Produce equivalent deterministic results across native Rust and WebAssembly.
- Support distributed compute where clients execute simulations and hosted
  services validate contracts, policies, and submitted artifacts.
- Keep domain solvers and simulators outside the reusable runtime boundary.
- Process captured operational telemetry for later comparison with simulation.
- Be usable through a CLI, composable Rust crates, and optional hosted services.

### 1.2 Non-Goals

Pitgun is **not**:

- A charting / dashboard tool (that’s downstream).
- A general data warehouse.
- A racing-only simulation stack.
- A universal physics Solver: each domain supplies its own mathematical model.
- A guarantee that a live external stream is deterministic.
- A stable implementation of the historical Pipeline, Analysis, Bolt, or Bundle
  prototype manifests.
- A monolithic platform: local execution does not require hosted services.

## 2. Workspace & Crate Layout

The framework is organized as a Rust workspace:
```
pitgun/
  ARCHITECTURE.md
  README.md
  crates/
    # Contract & Core
    pitgun-contract/      # domain-neutral frames, envelopes, contracts, registries
    pitgun-core/          # telemetry transforms and aggregation
    pitgun-policy/        # generic policy loading, canonicalization, constraints
    pitgun-signing/       # cryptographic signing utilities
    
    # Codecs
    pitgun-codec-udp/     # UDP binary wire format decoding
    pitgun-codec-json/    # SessionEnvelope JSON codec
    
    # Sources (all implement TelemetrySource trait)
    pitgun-source-udp/    # UDP unicast/multicast transport
    pitgun-source-ws/     # WebSocket client source
    pitgun-source-kafka/  # Kafka consumer source
    pitgun-source-mqtt/   # MQTT subscriber source

    # Deterministic runtime
    pitgun-runtime/       # RNG V1, linked execution and evidence verification

    # Racing domain family (migration in progress)
    pitgun-racing-contract/   # Racing wire and evidence schemas
    pitgun-racing-solver/     # Racing physical solution
    pitgun-racing-simulator/  # Racing orchestration, telemetry and WASM facade
    pitgun-racing-policy/     # Racing rules using the generic policy engine
  apps/
    pitgun-cli/           # deterministic demo, replay, optional live subscription
    pitgun-replay/        # replay tooling
  services/
    pitgun-gateway/       # framework ingress service
    pitgun-authority/     # signed contract and policy authority service
```

Deployment definitions intentionally do not appear in this repository map.
The workspace Dockerfile and `docker-compose.dev.yml` support local development;
`loicbelec/infra-vps` exclusively owns staging and production topology,
persistence, routing, and observability.

### 2.1 pitgun-contract

Responsibilities:

- `TelemetrySource` trait: common interface for all sources.
- `TelemetryFrame` model: canonical data format.
- `ParameterRegistry`: parameter definitions and metadata.
- Generic envelopes, signed contract payloads, schema/version identifiers.
- `SourceStats`, `SourceState`, `SourceError`: source lifecycle.
- `Sample`, `SampleValue`, `SignalQuality`: data types.

Target boundary: `pitgun-contract` should remain domain-neutral. Racing-specific
types currently present in the crate are migration candidates and should be
extracted or isolated so the generic crate surface does not encode racing
semantics.

### 2.2 pitgun-core

Responsibilities:

- Telemetry pipeline primitives and source/sink orchestration.
- `ConverterService`: raw to engineering unit conversion.
- Formula, filtering, scaling, statistics, and segment aggregation processors.
- Domain-neutral aggregation over simulated or observed channels.
- In-memory events, batches, and aggregate records.

### 2.3 pitgun-policy

Responsibilities:

- Load and validate policy definitions.
- Canonicalize submitted configuration payloads.
- Apply constraints and limits.
- Produce stable policy hashes.
- Support anti-cheat validation for distributed client-side simulation.

The engine should stay generic. Racing rules can be supplied as policy data, but
the reusable policy crate should not be hard-coded to `RaceInput`,
`CompetitorSpec`, or car setup structures.

### 2.4 pitgun-gateway

Responsibilities:

- Accept framework envelopes over transports such as WebSocket.
- Validate schema versions, contract references, signatures, sizes, and rates.
- Route accepted payloads to persistence or downstream consumers.

The gateway is the framework entrypoint. Racing fields belong in payloads or
metadata, not in the generic envelope.

The target names above describe the accepted migration boundary. The current
workspace still contains transitional `pitgun-solver` and `pitgun-simulator`
packages while the linked implementation issues are delivered.

### 2.5 pitgun-runtime

Responsibilities:

- Stable deterministic random streams.
- Deterministic execution context and logical ordering.
- Workload and model/version binding.
- Run and receipt verification orchestration.
- Comparison-profile dispatch and domain verifier hooks.

It is an execution runtime, not a universal numerical Solver. Versioned wire
types remain in `pitgun-contract`; filesystem and network adapters remain in
applications and services.

### 2.6 Racing domain family (migration in progress)

#### pitgun-racing-solver

Responsibilities:

- Solve the Racing vehicle and track physical problem.
- Own velocity, braking, acceleration, energy, and integration algorithms.
- Publish deterministic physical solution types and numerical invariants.

#### pitgun-racing-simulator

Responsibilities:

- Own race, session, lap, strategy, and event orchestration.
- Load and embed the racing data pack.
- Resolve racing ids such as `vehicle_id`, `track_id`, and `driver_id`.
- Produce Racing telemetry and expose the complete workload through WASM.
- Invoke `pitgun-racing-solver` when it needs a physical solution.

#### pitgun-racing-contract and pitgun-racing-policy

Racing schemas shared across processes and repositories live in
`pitgun-racing-contract`. Racing validation lives in `pitgun-racing-policy`,
which uses the generic policy engine.

The accepted rationale, dependency direction, static Rust/WASM integration
model, and migration table are defined by
[ADR 0001](docs/adr/0001-runtime-and-domain-workloads.md).

### 2.7 pitgun-cli

Responsibilities:

- Execute the built-in versioned Racing demonstration.
- Persist and verify portable deterministic Run Bundles.
- Replay an existing bundle in a fresh process.
- Expose optional telemetry subscription tooling while it remains supported.

### 2.8 Services (reference implementations)

Deployable binaries live under `services/`, separate from reusable crates. These
are intentionally thin wrappers around `pitgun-core` and future APIs.

## 3. Simulation and Observed-Data Dictionaries

Pitgun separates two kinds of semantics:

1. A domain simulation dictionary defines the physical quantities emitted by a
   versioned workload. Racing owns its own dictionary and wire contract.
2. An observed-data registry describes provider channels, units, conversions,
   quality metadata, and mappings needed to compare operations with a model.

Observed data must be captured before it can participate in reproducible
processing. `ParameterRegistry` can attach canonical names, units, ranges, and
conversions to provider parameter identifiers. Provider-specific names must not
leak into the deterministic runtime contract.

This makes it possible to apply the same aggregation or comparison logic across:

-  different cars,
-  different seasons,
-  different providers (motorsport vs infrastructure vs finance),

as long as an explicit, versioned mapping exists into the domain vocabulary.

## 4. Core Concepts

### 4.1 Channel

A processing channel is identified by a name and carries timestamped numeric
events. Rich typed simulation telemetry uses the separate `TelemetryFrame` and
`Sample` contract types.

- A generated channel name belongs to the workload's versioned output contract.
- An observed provider channel may be mapped to a canonical name through a
  registry before comparison.
- Units and valid ranges belong to the corresponding domain dictionary or
  observed-data registry.

The lightweight processing representation is:

```rust
pub struct Event {
    pub channel: String,
    pub ts_ns: u64,
    pub value: f64,
}
```

This lets processors remain agnostic to the original source while the richer
contract types retain identity, quality, and versioning at system boundaries.

### 4.2 Sample & Timeseries

A sample is a typed value inside a `TelemetryFrame`. Processing adapters may
project samples into numeric events. Alignment and interpolation are explicit
domain or processing decisions; the framework does not assume that live source
timestamps are already aligned.

### 4.3 Manifests

A manifest is a versioned, declarative identity or execution description. The
implemented public manifest today is the Run Bundle manifest: it binds a run to
its canonical scenario, contract, output, telemetry, metrics, and execution
receipt artifacts.

The repository previously contained Pipeline, Analysis, Bolt, and Bundle YAML
prototypes. They described useful research directions but did not form one
executable simulation lifecycle, and no compatibility guarantee was published.
They are not part of the current architecture.

A future run manifest may provide a user-authored entry point for model and
scenario selection, seeded execution, telemetry analysis, replay, and
verification. It must be designed from the implemented Run Contract and Run
Bundle invariants rather than evolved implicitly from those historical files.

### 4.4 Formula and aggregation processors

A formula consumes one or more channels and produces a derived channel or
metric. Aggregation processors summarize generated or captured streams across
explicit boundaries. These are reusable data-processing primitives, not a
substitute for the domain Solver or Simulator.

## 5. Manifest Lifecycle

Public manifests require:

- an owned Rust wire type;
- strict validation and unknown-field rejection;
- a published schema and lifecycle status;
- an executable producer and consumer;
- canonicalization rules when the manifest contributes to run identity;
- compatibility and migration rules for every new version.

Historical research formats that do not meet these requirements remain in Git
history. The current schema catalog labels superseded schema families as legacy.

## 6. Multi-Source Architecture

Pitgun supports ingesting telemetry from multiple heterogeneous sources through a unified pipeline architecture.

### 6.1 High-Level Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     TELEMETRY SOURCES                           │
│                   (Pluggable & Independent)                     │
└─────────────────────────────────────────────────────────────────┘
           │              │              │              │
    ┌──────┴────┐  ┌──────┴────┐  ┌──────┴────┐  ┌──────┴────┐
    │   UDP     │  │ WebSocket │  │   Kafka   │  │   MQTT    │
    │  Binary   │  │   JSON    │  │  Stream   │  │    IoT    │
    └───────────┘  └───────────┘  └───────────┘  └───────────┘
           │              │              │              │
           └──────────────┴──────────────┴──────────────┘
                          │
                   [TelemetryFrame]
                  (Canonical Format)
                          │
                          ▼
           ┌──────────────────────────────┐
           │   pitgun-contract            │
           │   - TelemetrySource trait    │
           │   - TelemetryFrame model     │
           │   - ParameterRegistry        │
           └──────────────────────────────┘
                          │
                          ▼
           ┌──────────────────────────────┐
           │   pitgun-core                │
           │   - Multi-source pipeline    │
           │   - Converter service        │
           │   - Formula engine           │
           └──────────────────────────────┘
```

### 6.2 TelemetrySource Trait

All sources implement a common trait defined in `pitgun-contract`:

```rust
#[async_trait]
pub trait TelemetrySource: Send + Sync {
    fn name(&self) -> &str;
    fn source_type(&self) -> SourceType;
    fn state(&self) -> SourceState;
    fn stats(&self) -> SourceStats;
    async fn start(&mut self, tx: UnboundedSender<TelemetryFrame>) -> Result<(), SourceError>;
    async fn stop(&mut self) -> Result<(), SourceError>;
}
```

Supported source types:
- **UDP** (`pitgun-source-udp`): Binary protocols, multicast support
- **WebSocket** (`pitgun-source-ws`): JSON-based real-time streams
- **Kafka** (`pitgun-source-kafka`): High-throughput streaming platform
- **MQTT** (`pitgun-source-mqtt`): IoT publish/subscribe protocol

### 6.3 TelemetryFrame

All sources emit a canonical `TelemetryFrame`:

```rust
pub struct TelemetryFrame {
    pub session_id: SessionId,
    pub timestamp_us: i64,
    pub sequence: u64,
    pub source_metadata: SourceMetadata,
    pub samples: Vec<Sample>,
    pub events: Vec<Event>,
}

pub struct Sample {
    pub parameter_id: u16,
    pub value: SampleValue,
    pub quality: SignalQuality,
    pub timestamp_offset_us: Option<i32>,
}
```

### 6.4 Multi-Source Pipeline

The `TelemetryPipeline` manages multiple sources concurrently:

```
┌──────────────────────────────────────────────────────────────┐
│                    TelemetryPipeline                         │
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ UDP Source  │  │  WS Source  │  │Kafka Source │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         │                │                │                  │
│         └────────────────┴────────────────┘                  │
│                          │                                   │
│                  [mpsc::channel]                             │
│                          │                                   │
│                          ▼                                   │
│                   ┌─────────────┐                            │
│                   │  Converter  │ → Raw to engineering units │
│                   └──────┬──────┘                            │
│                          │                                   │
│                          ▼                                   │
│                   ┌─────────────┐                            │
│                   │ Processors  │ → Transform and aggregate  │
│                   │             │                            │
│                   └─────────────┘                            │
└──────────────────────────────────────────────────────────────┘
```

### 6.5 ParameterRegistry

Observed-data parameters can be defined in YAML registries parsed by
`ParameterRegistry`:

```yaml
version: "1.0"
name: observed-racing-telemetry
parameters:
  - id: 1
    name: provider_engine_speed
    canonical_name: engine_speed
    unit: rpm
    data_type: U16
    conversion:
      type: linear
      scale: 1.0
      offset: 0.0
```

This registry is an integration concern. It does not define the physical
dictionary or output contract owned by a domain simulator.

### 6.6 Source Crates

| Crate | Transport | Use Case |
|-------|-----------|----------|
| `pitgun-source-udp` | UDP unicast/multicast | Binary telemetry, sensors |
| `pitgun-source-ws` | WebSocket | Games, web apps, JSON streams |
| `pitgun-source-kafka` | Kafka | High-volume data platforms |
| `pitgun-source-mqtt` | MQTT | IoT devices, pub/sub |

Each source crate implements `TelemetrySource` and uses the appropriate codec from `pitgun-codec-*`.

# Pitgun Architecture

Pitgun is a Rust framework for real-time telemetry ingestion, contract-governed
data exchange, manifest-driven processing, and distributed deterministic
compute.

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

- **Evaluate formulas at scale** on high-frequency time series (motorsport, infra, finance, energy, …).
- **Unify heterogeneous channels** into a **canonical dictionary**, independent of the original provider (F1 Atlas, NASDAQ feeds, cloud metrics, etc.).
- Provide a **manifest-driven engine**: pipelines and analyses are described as data (YAML / JSON), not hard-coded logic.
- Provide **contract-first ingestion** through signed envelopes, schemas, registries, and policy-controlled limits.
- Support **distributed deterministic compute** where a client can run a model locally and the server can verify the submitted configuration and outputs.
- Be **embeddable**:
  - as a CLI (`pitgun-cli`),
  - as a library (`pitgun-core`),
  - as deployable services such as `pitgun-gateway` and `pitgun-authority`.
- Offer a **clean path to productization**: bundle registry, versioned manifests, benchmarks, perf gates.
- **Ingest telemetry from multiple sources**: UDP, WebSocket, Kafka, MQTT with a unified pipeline.

### 1.2 Non-Goals

Pitgun is **not**:

- A charting / dashboard tool (that’s downstream).
- A general data warehouse.
- A racing-only simulation stack.
- A monolithic “platform”.  
  Instead, it’s a **small, composable core** focused on:
  - canonical channels,
  - formula evaluation,
  - manifest execution,
  - contract and policy enforcement,
  - domain-neutral ingress and compute verification.

## 2. Workspace & Crate Layout

The framework is organized as a Rust workspace:
```
pitgun/
  ARCHITECTURE.md
  README.md
  crates/
    # Contract & Core
    pitgun-contract/      # domain-neutral frames, envelopes, contracts, registries
    pitgun-core/          # formula engine, pipeline, converter, manifests
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

    # Deterministic runtime (migration in progress)
    pitgun-runtime/       # RNG V1 today; execution and verification target

    # Target Racing domain family
    pitgun-racing-contract/   # Racing wire and evidence schemas
    pitgun-racing-solver/     # Racing physical solution
    pitgun-racing-simulator/  # Racing orchestration, telemetry and WASM facade
    pitgun-racing-policy/     # Racing rules using the generic policy engine
  apps/
    pitgun-cli/           # CLI for running manifests locally
    pitgun-replay/        # replay tooling
  services/
    pitgun-gateway/       # framework ingress service
    pitgun-authority/     # signed contract and policy authority service
  examples/
    registries/
      motorsport_full.yaml
      iot_sensors.yaml
```

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

- `TelemetryPipeline`: multi-source orchestration.
- `ConverterService`: raw to engineering unit conversion.
- SAT JSON parsing + canonical dictionary resolution.
- Manifest models + validation hooks.
- FormulaProcessor v1: core evaluation loop.
- AST (Bolts) construction and execution.
- In-memory data model for channels and timeseries.

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

### 2.5 pitgun-runtime (target)

Responsibilities:

- Stable deterministic random streams.
- Deterministic execution context and logical ordering.
- Workload and model/version binding.
- Run and receipt verification orchestration.
- Comparison-profile dispatch and domain verifier hooks.

It is an execution runtime, not a universal numerical Solver. Versioned wire
types remain in `pitgun-contract`; filesystem and network adapters remain in
applications and services.

### 2.6 Racing domain family (target)

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

-  CLI entrypoint (`pitgun run`, `pitgun bench`, `pitgun inspect` etc).
-  Load manifests from disk.
-  Wire datasets (CSV, emulator, etc.) into the core.
-  Provide a UX for quick validation and experimentation.

### 2.8 Services (reference implementations)

Deployable binaries live under `services/`, separate from reusable crates. These
are intentionally thin wrappers around `pitgun-core` and future APIs.

## 3. Canonical Dictionary & SAT JSON

Pitgun separates two concerns:

1.	**Provider dictionaries** (e.g. Atlas, NASDAQ, Datadog):
    -  Raw channel names, raw units, raw quirks.
2.	A **canonical dictionary:**
    -  Stable names (engine_speed, brake_pressure_front_left, lap_phase),
    -  Domain semantics (motorsport, infra, finance, energy),
    -  Units, sampling conventions, constraints.

When ingesting data:
1.	A SAT JSON file describes how provider channels map into the canonical dictionary.
2.	The ingestion layer builds a canonical view of all channels.
3.	The formula engine only sees canonical names, never "FIA-nEngine" or "cpu_load_15".

Example SAT JSON excerpt:

```json
{
  "provider": "atlas",
  "session_type": "race",
  "mappings": [
    {
      "provider_name": "FIA-nEngine",
      "canonical_name": "engine_speed",
      "unit": "rpm"
    },
    {
      "provider_name": "FIA-ThrottlePedal",
      "canonical_name": "throttle_pedal_ratio",
      "unit": "%"
    }
  ]
}
```

This makes it possible to reuse the same analysis manifest across:

-  different cars,
-  different seasons,
-  different providers (motorsport vs infra vs finance), 
as long as there is a SAT mapping into the canonical space.

## 4. Core Concepts

### 4.1 Channel

A channel is a time series identified by a name and metadata:

- A **canonical name** (engine_speed, throttle, norm_acc, etc.).
- A **provider-specific name** (e.g. FIA-nEngine, Car_Speed, cpu_usage).
- Units, sampling rate, domain (motorsport, infra, finance…), tags.

Internally, channels are described using a **SAT JSON** schema:

```json
{
  "canonical_name": "engine_speed",
  "provider": "atlas",
  "provider_name": "FIA-nEngine",
  "unit": "rpm",
  "domain": "motorsport",
  "sampling_hz": 1000,
  "tags": ["powertrain", "telemetry"]
}
```

The canonical layer allows the engine to stay agnostic to the original source.

### 4.2 Sample & Timeseries

A **sample** is a (timestamp, value) pair; a **timeseries** is a vector/array of samples.

Pitgun assumes:

-  **aligned timestamps** across channels for a given session/lap (or uses interpolation when configured),
-  numeric values (float / integer) for the first versions.

### 4.3 Manifest

A **manifest** is a declarative description of what the engine should do.

Pitgun introduces several manifest types:

-  **Pipeline manifest:** how to ingest, route and pre-process channels.
-  **Analysis manifest:** which formulas to apply, in which order, with which inputs.
-  **Bundle manifest:** how formulas are grouped by topic (tyres, engine, infra, trading…).
-  **Bolt manifest:** AST description of a single formula.

All manifests are versioned and validated against published schemas.

### 4.4 Formula, Bolt, Bundle

-  A **Formula** is a computation that consumes one or more channels and produces a new channel or metric.
-  A **Bolt** is the **AST representation** of a formula (the low-level, engine-friendly form).
-  A **Bundle** is a toolbox: a curated set of related formulas (e.g. “tyre degradation”, “engine monitoring”, “CPU health”).

## 5. Manifest Types

### 5.1 Pipeline Manifest

Describes how data flows through Pitgun:

-  sources (UDP, gRPC, Kafka, CSV…),
-  processors (filters, resamplers, normalizers),
-  sinks (exporters, API hooks…).

Example (simplified):

```yaml
version: v1
kind: pipeline
sources:
  - name: atlas_udp
    type: udp
    host: 0.0.0.0
    port: 20777
    sat_file: ./sat/atlas_race.json

processors:
  - name: baseline_filters
    type: standard_filters
    config:
      drop_nan: true
      low_pass_cutoff_hz: 100

sinks:
  - name: stdout_debug
    type: stdout
    channels:
      - engine_speed
      - throttle_pedal_ratio
```

### 5.2 Analysis Manifest

Describes the higher-level physics / math layer:

-  Which bundles are used.
-  Which formulas to run.
-  Dependencies between formulas.

```yaml
version: v1
kind: analysis
name: engine_health_v1
bundles:
  - engine_core
  - engine_anomaly_detection

graph:
  - formula: engine_speed_norm
    inputs: [engine_speed]
  - formula: throttle_smoothness
    inputs: [throttle_pedal_ratio]
  - formula: engine_stress_index
    inputs:
      - engine_speed_norm
      - throttle_smoothness
```

### 5.3 Bundle Manifest

Describes a bundle as a toolbox of formulas:

```yaml
version: v1
kind: bundle
name: engine_core
description: Core engine metrics (normalized speed, torque proxies, etc.)
formulas:
  - engine_speed_norm
  - engine_speed_ramp
  - engine_overrev_events
```

### 5.4 Bolt Manifest (Formula AST)

Describes a single formula in AST form, consumable by FormulaProcessor v1:

```yaml
{
  "version": "v1",
  "kind": "bolt",
  "name": "engine_speed_norm",
  "inputs": ["engine_speed"],
  "output": "engine_speed_norm",
  "ast": {
    "type": "BinaryOp",
    "op": "Div",
    "left": { "type": "Input", "name": "engine_speed" },
    "right": { "type": "Constant", "value": 18000.0 }
  }
}
```

Later, FormulaEngine v2 can evolve this AST without breaking the higher-level manifests.

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
│                   │   Formula   │ → Apply analysis manifests │
│                   │   Engine    │                            │
│                   └─────────────┘                            │
└──────────────────────────────────────────────────────────────┘
```

### 6.5 ParameterRegistry

Parameters are defined in YAML registries:

```yaml
version: v1
domain: motorsport
parameters:
  - id: 0x0001
    name: engine_speed
    unit: rpm
    data_type: u16
    conversion:
      formula: linear
      scale: 1.0
      offset: 0.0
    
  - id: 0x0002
    name: throttle_position
    unit: percent
    data_type: u8
    conversion:
      formula: linear
      scale: 0.392157  # 100/255
      offset: 0.0
```

### 6.6 Source Crates

| Crate | Transport | Use Case |
|-------|-----------|----------|
| `pitgun-source-udp` | UDP unicast/multicast | Binary telemetry, sensors |
| `pitgun-source-ws` | WebSocket | Games, web apps, JSON streams |
| `pitgun-source-kafka` | Kafka | High-volume data platforms |
| `pitgun-source-mqtt` | MQTT | IoT devices, pub/sub |

Each source crate implements `TelemetrySource` and uses the appropriate codec from `pitgun-codec-*`.

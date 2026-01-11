# Pitgun Architecture

Pitgun is a high-performance formula engine and telemetry framework.  
It ingests raw signals (telemetry, infra metrics, etc.), applies manifest-driven processing, and emits new channels and metrics in real time.

This document describes the **core architecture** of the Pitgun framework (Rust workspace), its **key concepts**, and how the different crates collaborate.

## 1. Goals & Non-Goals

### 1.1 Goals

Pitgun is designed to:

- **Evaluate formulas at scale** on high-frequency time series (motorsport, infra, finance, energy, …).
- **Unify heterogeneous channels** into a **canonical dictionary**, independent of the original provider (F1 Atlas, NASDAQ feeds, cloud metrics, etc.).
- Provide a **manifest-driven engine**: pipelines and analyses are described as data (YAML / JSON), not hard-coded logic.
- Be **embeddable**:
  - as a CLI (`pitgun-cli`),
  - as a library (`pitgun-core`),
  - later as a service behind `api.pitgun.io`.
- Offer a **clean path to productization**: bundle registry, versioned manifests, benchmarks, perf gates.

### 1.2 Non-Goals

Pitgun is **not**:

- A charting / dashboard tool (that’s downstream).
- A general data warehouse.
- A monolithic “platform”.  
  Instead, it’s a **small, composable core** focused on:
  - canonical channels,
  - formula evaluation,
  - manifest execution.

## 3. Core Concepts

### 3.1 Channel

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

### 3.2 Sample & Timeseries

A **sample** is a (timestamp, value) pair; a **timeseries** is a vector/array of samples.

Pitgun assumes:

-  **aligned timestamps** across channels for a given session/lap (or uses interpolation when configured),
-  numeric values (float / integer) for the first versions.

### 3.3 Manifest

A **manifest** is a declarative description of what the engine should do.

Pitgun introduces several manifest types:

-  **Pipeline manifest:** how to ingest, route and pre-process channels.
-  **Analysis manifest:** which formulas to apply, in which order, with which inputs.
-  **Bundle manifest:** how formulas are grouped by topic (tyres, engine, infra, trading…).
-  **Bolt manifest:** AST description of a single formula.

All manifests are versioned and validated against published schemas.

### 3.4 Formula, Bolt, Bundle

-  A **Formula** is a computation that consumes one or more channels and produces a new channel or metric.
-  A **Bolt** is the **AST representation** of a formula (the low-level, engine-friendly form).
-  A **Bundle** is a toolbox: a curated set of related formulas (e.g. “tyre degradation”, “engine monitoring”, “CPU health”).

## 4. Workspace & Crate Layout

The framework is organized as a Rust workspace:
```
pitgun/
  ARCHITECTURE.md
  README.md
  crates/
    pitgun-core/       # formula engine, manifests, canonical model
    pitgun-emulator/   # dataset playback and synthetic channels (optional)
    pitgun-codec-udp/  # UDP wire decoding (Pitgun v1)
    pitgun-source-udp/ # UDP transport source
    pitgun-codec-json/ # SessionEnvelope JSON codec
    pitgun-source-ws/  # WebSocket client source
    pitgun-registry/   # local view of bundles & bolts (optional / future)
  apps/
    pitgun-cli/        # CLI for running manifests locally
  services/
    pitgun-telemetryd/ # telemetry ingestion service (reference implementation)
    pitgun-configd/    # config authority service (reference implementation)
  examples/
    manifests/
      pipeline/
      analysis/
    registry/
      bundles/
```

### 4.1 pitgun-core

Responsibilities:

-  SAT JSON parsing + canonical dictionary resolution.
-  Manifest models + validation hooks.
-  FormulaProcessor v1: core evaluation loop.
-  AST (Bolts) construction and execution.
-  In-memory data model for channels and timeseries.

### 4.2 pitgun-cli

Responsibilities:

-  CLI entrypoint (`pitgun run`, `pitgun bench`, `pitgun inspect` etc).
-  Load manifests from disk.
-  Wire datasets (CSV, emulator, etc.) into the core.
-  Provide a UX for quick validation and experimentation.

### 4.3 pitgun-emulator (optional)

Responsibilities:

-  Load recorded datasets (CSV, Parquet…).
-  Replay them at configurable speed (real-time, xN, as-fast-as-possible).
-  Provide synthetic/sandbox channels to test formulas and manifests locally.

### 4.4 Services (reference implementations)

Deployable binaries live under `services/`, separate from reusable crates. These
are intentionally thin wrappers around `pitgun-core` and future APIs.

## 5. Canonical Dictionary & SAT JSON

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

## 6. Manifest Types

### 6.1 Pipeline Manifest

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

### 6.2 Analysis Manifest

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

### 6.3 Bundle Manifest

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

### 6.4 Bolt Manifest (Formula AST)

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

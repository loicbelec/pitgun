[![Pitgun](docs/img/pitgun_transparent.png)](https://pitgun.io)

# Pitgun: Distributed Intelligence Mesh

> **From Raw Telemetry to Distributed Decision Making.**

Pitgun is a modular Rust framework designed to ingest high-frequency data streams, apply dynamic engineering logic, and orchestrate distributed computations at the edge.

While currently showcasing a **Reference Implementation in Motorsport** (F1 Simulation), its architecture is domain-agnostic and built for Finance, Energy, and IoT reliability.

---

## 🏗️ The Architecture

Pitgun is built on four pillars that separate **Ingestion**, **Processing**, **Compute**, and **Governance**.

### 1. 📡 The Gateway (Ingestion & Traffic)
A high-throughput ingestion layer capable of normalizing diverse protocols into a single unified `TelemetryFrame`.
*   **Multi-Protocol:** Native support for UDP (Unicast/Multicast), WebSocket, Kafka, and MQTT.
*   **Normalization:** Translates disparate wire formats (binary, JSON) into a strict internal schema.
*   **Service:** `services/pitgun-gateway`

### 2. ⚡ The Core (Dynamic Processing)
A powerful **Manifest-Driven ETL engine** that allows engineers to define derived channels without recompiling code.
*   **Formula Engine:** Define `Power = Torque * RPM` using an AST-based expression parser (`pitgun-core`).
*   **Manifests:** YAML-based configuration for pipelines (`channel_filter`, `scale`, `segment_aggregate`).
*   **Registry:** Strictly typed parameter definitons (`u16`, `f64`) with validation ranges.

### 3. 🧠 The Solver (Distributed Compute)
An orchestration layer for offloading complex optimization tasks to an edge grid (e.g., WebAssembly Clients).
*   **Use Cases:** Monte Carlo Simulations, Risk Analysis, Pathfinding.
*   **Technology:** Rust -> WASM compilation for browser-based volunteer computing.
*   **Service:** `crates/pitgun-solver`

### 4. ⚖️ The Authority (Governance)
A security layer ensuring that data and configurations are authentic and tamper-proof.
*   **Access Control:** Rate limiting and capability-based access (`pitgun-policy`).
*   **Policy Enforcement:** Cryptographic signing of simulation contracts (Tuning Limits).
*   **Auditability:** Guarantees that result A came from Config B.
*   **Service:** `services/pitgun-authority`

---

## 🧱 Component Stack

### Foundation Crates
| Crate | Role | Description |
|-------|------|-------------|
| **pitgun-core** | **The Brain** | AST Formula Engine, Pipeline logic, Manifest parsing. |
| **pitgun-contract** | **The Law** | Shared types (`TelemetryFrame`), IDL, and protocols. |
| **pitgun-engine-f1** | **Ref. Impl** | A deterministic Physics Engine (Data Plane) for F1 simulation. |
| **pitgun-solver** | **Control Plane** | Strategy & Risk optimization logic skeleton. |

### Infrastructure
| Service | Role | Container |
|---------|------|-----------|
| **pitgun-gateway** | Traffic Ingress | `pitgun-gateway` |
| **pitgun-authority** | Security/Config | `pitgun-authority` |
| **pitgun-replay** | Tooling | `apps/pitgun-replay` |

---

## 🚀 Quickstart

### 1. Define your Logic (The Manifest)
Create a `pipeline.yaml` to define how data should be processed dynamically:

```yaml
version: v1
pipeline:
  - type: formula
    derived_channels:
      - name: "Power_kW"
        expr: "Torque_Nm * Engine_RPM / 9549.0"
  - type: filter
    whitelist: ["Speed", "Power_kW", "LapTime"]
```

### 2. Start the Gateway
```bash
cargo run -p pitgun-gateway --release
```

### 2b. Start Services with Docker Compose (Dev)
```bash
docker compose -f docker-compose.dev.yml up -d
```

Production compose is maintained in the dedicated infra repository; this repo ignores
`docker-compose.prod.yml`.

### 3. Inject Data (Replay)
Simulate a stream of data using the replay tool:
```bash
cargo run -p pitgun-replay -- \
  --target 127.0.0.1:8080 \
  --input nEngine=datasets/telemetry/nEngine.csv
```

---

## 📚 Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Deep dive into the hexagonal architecture.
- [docs/WIRE_FORMATS.md](docs/WIRE_FORMATS.md) - Wire protocol specifications.
- [policies/gametuning.v1.yaml](policies/gametuning.v1.yaml) - Example of Governance Policy.

## ✅ Local CI Before Commit

Run the same checks as `pitgun-ci` locally before pushing:

```bash
./scripts/pre-commit-checks.sh
```

## 🔮 Roadmap

- **WASM Solver:** Complete the Monte Carlo implementation for the Solver.
- **Parquet Sink:** Archival storage for historical analysis.
- **Flow UI:** Visual editor for `pitgun-core` pipeline manifests.

---

> Built with 🦀 Rust for performance and safety.

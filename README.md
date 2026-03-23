[![Pitgun](docs/img/pitgun_transparent.png)](https://pitgun.io)

# Pitgun Framework

> High-performance Rust framework for real-time data ingestion, deterministic processing, and policy-governed distributed compute.

Pitgun Framework helps engineering teams build reliable event pipelines and decision systems from heterogeneous telemetry and data streams.
It is domain-agnostic by design (industrial, IoT, mobility, finance, and simulation-heavy workloads).

## What You Get

- Multi-protocol ingestion (UDP, WebSocket, Kafka, MQTT)
- Strict contracts and typed schemas for event safety
- Manifest-driven processing pipelines (no full recompilation for every rule change)
- Deterministic simulation and compute primitives
- Replay tooling for debugging and reproducibility
- Governance and policy controls for access and signed configurations

## Architecture At A Glance

| Layer | Responsibility | Main Components |
|---|---|---|
| Sources and Codecs | Connect to external systems and normalize inputs | `pitgun-source-*`, `pitgun-codec-*` |
| Gateway | Receive, validate, and persist incoming envelopes | `services/pitgun-gateway` |
| Core Processing | Transform and derive channels with manifest-defined logic | `crates/pitgun-core` |
| Contracts | Shared schemas and protocol types | `crates/pitgun-contract` |
| Policy and Signing | Validation, access control, signing primitives | `crates/pitgun-policy`, `crates/pitgun-signing` |
| Solver and Compute | Deterministic compute kernel plus simulator data/runtime adapter | `crates/pitgun-solver`, `crates/pitgun-simulator` |
| Tooling | Replay and CLI operations | `apps/pitgun-replay`, `apps/pitgun-cli` |
| Authority Service | Governance-facing runtime service | `services/pitgun-authority` |

## Repository Layout

```text
crates/     # reusable framework crates
apps/       # operator and developer tools
services/   # deployable runtime services
docs/       # protocol, architecture, and technical documentation
examples/   # manifests, registries, and integration examples
policies/   # policy samples
```

## Quickstart

### Prerequisites

- Rust stable toolchain
- Cargo
- Optional: Docker for local service stack

### 1) Build the workspace

```bash
cargo check --workspace
```

### 2) Run the gateway locally

```bash
PITGUN_GATEWAY_API_KEY=dev-secret \
PITGUN_GATEWAY_BIND=127.0.0.1:8080 \
cargo run -p pitgun-gateway --release
```

### 3) Verify health

```bash
curl -fsS http://127.0.0.1:8080/health
```

### 4) Send a sample envelope (optional)

```bash
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/session.start.json
```

## Example: Manifest-Driven Processing

```yaml
version: v1
pipeline:
  - type: formula
    derived_channels:
      - name: "derived.metric"
        expr: "source.a * source.b"
  - type: filter
    whitelist: ["derived.metric", "source.timestamp"]
```

## Configuration

Runtime behavior is controlled by environment variables.
For gateway-specific variables and payload contracts, see:

- `services/pitgun-gateway/README.md`
- `services/pitgun-gateway/docs/event-model.md`

## Documentation Map

- `ARCHITECTURE.md` - framework architecture and boundaries
- `docs/SOLVER_SIMULATOR_BOUNDARY.md` - explicit boundary between compute kernel and simulator adapter
- `docs/WIRE_FORMATS.md` - wire protocol specifications
- `docs/commands.md` - CLI and command usage
- `docs/index.md` - entry point for technical docs

## Security and Privacy Principles

- Data minimization by default
- Contract-first validation at ingress boundaries
- Policy-gated sensitive operations
- Signed configuration paths for auditability
- Pseudonymous identifiers recommended for production telemetry

## Quality Gate (Before Commit)

Run the same checks as CI before pushing:

```bash
./scripts/pre-commit-checks.sh
```

If Docker is installed, this script also performs a local gateway image build equivalent to `build-gateway.yml`.

## Notes On Domain Neutrality

This repository may include reference assets and examples from specific domains.
Those examples demonstrate usage patterns only; the framework primitives remain domain-agnostic.

## License

MIT

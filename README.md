[![Pitgun](docs/img/pitgun_transparent.png)](https://pitgun.com)

# Pitgun

[![CI](https://github.com/loicbelec/pitgun/actions/workflows/pitgun-ci.yml/badge.svg)](https://github.com/loicbelec/pitgun/actions/workflows/pitgun-ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> Deterministic simulation to telemetry, in Rust.

Pitgun is an experimental framework for building, running, observing, and
replaying deterministic time-series simulations. It connects the whole loop:

**Model → Simulate → Observe → Replay and verify**

Racing is Pitgun's first reference application and proving ground. The framework
is designed to become useful beyond motorsport wherever reproducible simulations,
event streams, and auditable results matter.

[Website](https://pitgun.com) · [Developer blueprints](https://pitgun.dev) · [Play Racing](https://play.pitgun.com)

> [!IMPORTANT]
> Pitgun is an alpha-stage personal R&D project. Its deterministic contracts are
> being stabilized, but APIs and crate boundaries may still change.

## The Simulation Loop

| Stage | Pitgun responsibility |
|---|---|
| **Model** | Define versioned domain inputs, contracts, registries, and physical parameters. |
| **Simulate** | Execute seeded logic consistently across native Rust and WebAssembly. |
| **Observe** | Emit typed events and telemetry through reusable ingestion and processing components. |
| **Replay and verify** | Preserve run identity and artifacts so results can be reproduced and compared. |

The long-term objective is not a universal physics engine. Pitgun provides the
deterministic execution and telemetry infrastructure; each application supplies
its own domain model and physical rules.

## What Exists Today

- A [deterministic run contract](docs/DETERMINISTIC_RUN_CONTRACT_V1.md) covering
  run identity, replay inputs, artifacts, and native/WASM comparison
- A versioned [stable RNG contract](docs/RNG_V1.md) with independently derived
  random streams
- A Racing golden scenario exercised in both native Rust and Node/WASM
- Racing physics, lap simulation, data packs, and browser-compatible WASM
- Domain-neutral envelopes, contracts, manifests, sources, codecs, and gateway
  ingestion
- Replay, command-line, policy, signing, and authority building blocks

Exact cross-runtime output receipts are the next deterministic milestone; see
[#55](https://github.com/loicbelec/pitgun/issues/55). Until that lands, the golden
scenario validates a shared compact summary rather than the complete telemetry
digest contract.

## Try the Deterministic Boundary

### Prerequisites

- A stable Rust toolchain
- Cargo
- Optional: Node.js and `wasm-pack` for the WASM check

Clone the repository and run the native Racing golden scenario:

```bash
git clone https://github.com/loicbelec/pitgun.git
cd pitgun
cargo test -p pitgun-solver --test racing_golden
```

Run the corresponding Node/WASM test:

```bash
cargo install wasm-pack --locked --version 0.14.0
wasm-pack test --node crates/pitgun-solver
```

For the entire Rust workspace:

```bash
cargo test --all
```

The planned developer entry point is a single command such as
`pitgun demo racing --seed 42`. It is tracked in
[#49](https://github.com/loicbelec/pitgun/issues/49) and is not available yet.

## Framework and Racing

| | Framework | Racing |
|---|---|---|
| **Role** | Reusable deterministic simulation, telemetry, replay, and governance infrastructure | Reference application and realistic telemetry generator |
| **Owns** | Execution contracts, envelopes, pipelines, manifests, run identity, verification primitives | Cars, circuits, setups, strategies, lap physics, and race orchestration |
| **Purpose** | Support multiple deterministic time-series domains | Prove the framework against a concrete, engaging domain |

Motorsport remains central as the showcase: it makes simulation results visible,
creates useful datasets, and continuously tests native/WASM portability. It is
not intended to define the framework's generic abstractions.

## Solver and Simulator

Pitgun deliberately preserves two different responsibilities:

| Component | Responsibility |
|---|---|
| **Solver** | Deterministic execution, stable randomness, run identity, hashing, and verification primitives |
| **Simulator** | Domain state, physical rules, time evolution, events, and application-specific outputs |

The repository is currently migrating toward this boundary. Some Racing golden
logic still lives in `pitgun-solver`; the target separation is documented in
[Framework Boundaries](docs/FRAMEWORK_BOUNDARIES.md). The README describes both
the current implementation and the intended direction rather than presenting the
migration as complete.

## Architecture at a Glance

| Layer | Responsibility | Main components |
|---|---|---|
| Sources and codecs | Connect external systems and normalize inputs | `pitgun-source-*`, `pitgun-codec-*` |
| Gateway | Receive, validate, and route generic data envelopes | `services/pitgun-gateway` |
| Core processing | Transform channels with manifest-defined logic | `crates/pitgun-core` |
| Contracts | Define envelopes, frames, registries, and signed contracts | `crates/pitgun-contract` |
| Policy and signing | Evaluate policies, canonicalize, constrain, and sign | `crates/pitgun-policy`, `crates/pitgun-signing` |
| Deterministic compute | Execute and verify reproducible runs | `crates/pitgun-solver` |
| Racing application | Model lap physics, orchestrate races, and expose WASM | `crates/pitgun-simulator` |
| Tooling | Operate and replay data flows | `apps/pitgun-cli`, `apps/pitgun-replay` |
| Authority service | Expose governance-facing runtime operations | `services/pitgun-authority` |

```text
crates/     reusable framework and simulation crates
apps/       operator and developer tools
services/   deployable runtime services
docs/       contracts, architecture, and technical documentation
examples/   manifests, registries, and integration examples
policies/   policy samples
```

## Optional: Run the Gateway

The gateway demonstrates Pitgun's generic telemetry ingress. It is one component
of the loop, not the primary product entry point.

```bash
PITGUN_GATEWAY_API_KEY=dev-secret \
PITGUN_GATEWAY_BIND=127.0.0.1:8080 \
cargo run -p pitgun-gateway --release
```

```bash
curl -fsS http://127.0.0.1:8080/health
```

Gateway payloads and configuration are documented in
[`services/pitgun-gateway`](services/pitgun-gateway/README.md).

## Roadmap

The current sequence is intentionally proof-driven:

1. Stabilize deterministic contracts and randomness — implemented in v1.
2. Produce exact native/WASM run and output digests —
   [#55](https://github.com/loicbelec/pitgun/issues/55).
3. Package the proof as an under-five-minute Racing demo —
   [#49](https://github.com/loicbelec/pitgun/issues/49).
4. Extract the domain-neutral compute kernel while keeping Racing as the
   reference implementation.
5. Apply the same loop to a second domain before claiming generality.

## Documentation

- [Architecture](ARCHITECTURE.md) — components, data flow, and ownership
- [Framework boundaries](docs/FRAMEWORK_BOUNDARIES.md) — generic and Racing separation
- [Deterministic run contract v1](docs/DETERMINISTIC_RUN_CONTRACT_V1.md) — identity, reproducibility, and replay
- [Stable RNG v1](docs/RNG_V1.md) — generator and stream derivation algorithms
- [Wire formats](docs/WIRE_FORMATS.md) — protocol specifications
- [Command reference](docs/commands.md) — current CLI usage
- [Documentation index](docs/index.md) — complete technical map

The visual architecture blueprints at [pitgun.dev](https://pitgun.dev) complement
these repository-level contracts.

## Contributing

Issues and focused pull requests are welcome. Before pushing, run the same local
quality gate used by the project:

```bash
./scripts/pre-commit-checks.sh
```

CI protects both the general build and the native/WASM golden boundary.

## License

Pitgun Framework is available under the [MIT License](LICENSE).

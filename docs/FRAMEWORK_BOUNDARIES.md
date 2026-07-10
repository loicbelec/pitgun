# Framework Boundaries

Pitgun is a generic telemetry and distributed deterministic compute framework.
The racing game is its first production domain and demonstration workload.

This document defines the target boundaries used when adding code, opening PRs,
and reviewing future architecture changes.

## Generic Framework

Generic framework crates must not depend on racing semantics. They may carry
racing data as payloads, samples, metadata, schemas, or registry entries, but
they should not need to understand concepts such as vehicles, tires, circuits,
lap times, pit stops, or race standings.

### `pitgun-contract`

Owns domain-neutral contracts and data shapes:

- telemetry frames and samples
- source and codec traits
- parameter registries
- generic event envelopes
- signed contract payloads
- schema and version identifiers

Target direction: extract racing types such as `RaceInput`, `RaceOutput`,
`CompetitorSpec`, `TuningSpec`, racing catalog entries, and lap-specific frame
fields from the generic crate surface.

### `pitgun-core`

Owns reusable processing primitives:

- event batches
- pipelines
- formula and manifest execution
- conversion and derived-channel processing

It should process data described by contracts and registries without coupling to
the racing game.

### `pitgun-policy`

Owns the generic policy engine:

- policy loading
- canonicalization
- constraints
- policy hashing
- payload validation primitives

Domain policies can define racing rules, but the engine itself should not be
hard-coded to `RaceInput` or player car setup structures.

### `pitgun-signing`

Owns signing and verification primitives for contracts and authority-issued
configuration.

### `pitgun-gateway`

Owns framework ingress:

- transport adapters, with WebSocket as the first production transport
- generic envelope parsing
- schema and contract validation
- rate and size limits
- routing to persistence or downstream consumers

The gateway can receive racing events, but racing fields should live in payloads
or metadata rather than in the generic envelope model.

### `pitgun-solver`

Target role, if retained: generic deterministic compute and verification.

It should focus on concepts such as:

- deterministic job identity
- canonical input and output hashing
- model/version identifiers
- result verification hooks
- reusable execution contracts

It should not own racing physics or racing telemetry semantics.

## Racing Domain

### `pitgun-simulator`

Owns the racing simulator:

- lap-time and race simulation
- vehicle, engine, tire, aero, chassis, track, driver, pit stop, and hybrid
  energy models
- racing data pack loading and embedded WASM distribution
- mapping racing asset ids to resolved simulation inputs
- racing telemetry generated from simulation output

This crate is the right home for racing model code currently living in
`pitgun-solver`.

## Distributed Simulation Flow

The intended product flow is:

1. The authority service issues a signed simulation contract.
2. The client receives the contract and runs the racing simulator locally.
3. The client submits configuration, outputs, telemetry summaries, and contract
   references through the gateway.
4. The gateway validates the generic envelope and contract limits.
5. The policy engine verifies that the configuration is canonical and allowed.
6. Server-side services verify or audit deterministic outputs as needed.

This keeps heavy simulation work distributed while preserving server-side
control over contracts, policies, and accepted data.

## Review Checklist

Before merging a framework change, ask:

- Does a generic crate need to understand a racing concept?
- Could this be represented as a schema, payload, registry entry, or metadata?
- Is the policy engine generic, with domain rules supplied as data?
- Is the gateway acting as framework ingress rather than a game API?
- Is racing simulation behavior isolated in `pitgun-simulator`?

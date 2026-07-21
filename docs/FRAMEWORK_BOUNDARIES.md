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
- formula and segment-aggregation execution
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

### `pitgun-runtime`

Owns domain-neutral deterministic execution:

- stable seeded random streams
- deterministic execution context and logical ordering
- workload and model/version identity binding
- run and receipt verification orchestration
- comparison-profile dispatch and domain verifier hooks

It does not contain a universal numerical Solver, domain equations, filesystem
persistence, network transports, or CLI presentation. The first workload
interface links Rust implementations at compile time; loading external WASM
plugins is a later, separately governed capability.

The crate owns RNG V1, the statically linked workload boundary, and pure loaded
Run Bundle verification. The workload lifecycle is documented in
[Statically Linked Workloads](LINKED_WORKLOAD.md); the storage boundary is
documented in [Loaded Run Bundle Verification](RUN_BUNDLE_VERIFICATION.md).

### `pitgun-gateway`

Owns framework ingress:

- transport adapters, with WebSocket as the first production transport
- generic envelope parsing
- schema and contract validation
- rate and size limits
- routing to persistence or downstream consumers

The gateway can receive racing events, but racing fields should live in payloads
or metadata rather than in the generic envelope model.

## Racing Domain

The Racing implementation uses explicit domain-prefixed crates.
`pitgun-racing-contract` owns the domain schemas and Racing consumers import it
directly. The current `pitgun-solver` and `pitgun-simulator` packages remain
transitional until their migration issues land.

### `pitgun-racing-contract`

Owns Racing input, output, evidence, catalog, and cross-process payload schemas.
It may build on generic contract identifiers and telemetry frames without
adding Racing fields to `pitgun-contract`.

### `pitgun-racing-solver` (target)

Owns the Racing physical and mathematical solution:

- vehicle, engine, tire, aero, chassis, track, driver, and hybrid energy models
- velocity, braking, acceleration, energy, and integration algorithms
- deterministic physical solution types and numerical invariants

It does not orchestrate races, strategies, sessions, or leaderboards.

### `pitgun-racing-simulator` (target)

Owns the racing simulator:

- race, session, lap, competitor, strategy, pit stop, and event orchestration
- racing data pack loading and embedded WASM distribution
- mapping racing asset ids to resolved simulation inputs
- racing telemetry generated from simulation output
- the linked workload adapter used by `pitgun-runtime`

It intentionally depends on `pitgun-racing-solver`; the reverse dependency is
forbidden.

### `pitgun-racing-policy`

Owns Racing setup canonicalization and validation while delegating generic
policy loading, constraints, and hashing to `pitgun-policy`.

The complete decision and dependency graph are fixed by
[ADR 0001](adr/0001-runtime-and-domain-workloads.md).

## Distributed Simulation Flow

The intended product flow is:

1. The authority service issues a signed simulation contract.
2. The client receives the contract and the generic runtime executes the linked
   Racing workload locally.
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
- Is generic execution logic owned by `pitgun-runtime` rather than a domain
  Solver?
- Are the domain Solver and Simulator still separate, with the Simulator
  depending on the Solver only?
- Would a proposed generic abstraction still make sense for a materially
  different second domain?

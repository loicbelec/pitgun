# ADR 0001: Generic Runtime and Domain Workloads

- Status: Accepted
- Date: 2026-07-18
- Decision issue: [#41](https://github.com/loicbelec/pitgun/issues/41)
- Parent epic: [#46](https://github.com/loicbelec/pitgun/issues/46)

## Context

The current `pitgun-solver` crate combines several different responsibilities:

- stable deterministic random primitives;
- Racing vehicle and track physics;
- spatial and temporal solution algorithms;
- race and session orchestration;
- Racing output and evidence types;
- native and WebAssembly exports.

Earlier target documentation proposed turning that crate into a generic Solver
while moving Racing behavior into `pitgun-simulator`. That direction creates two
problems.

First, a Solver is normally an implementation that solves one class of domain
models. A Racing velocity-profile solver and an electrical load-flow solver do
not share a universal solution algorithm merely because both execute
deterministically.

Second, moving all Racing physics into one Simulator crate would erase the
intentional distinction between solving a physical problem and orchestrating a
system through time.

Pitgun needs a generic execution boundary without claiming that the Racing
equations are universal.

## Decision

Pitgun separates the generic deterministic runtime from domain-specific
workloads. Solver and Simulator remain distinct concepts and distinct crates
inside each domain.

The target crate families are:

```text
Generic framework
├── pitgun-contract
├── pitgun-runtime
├── pitgun-core
├── pitgun-policy
└── pitgun-signing

Racing domain
├── pitgun-racing-contract
├── pitgun-racing-solver
├── pitgun-racing-simulator
└── pitgun-racing-policy

Future example
├── pitgun-grid-contract
├── pitgun-grid-solver
└── pitgun-grid-simulator
```

The future Grid names illustrate the ownership rule; this ADR does not authorize
or schedule a Grid implementation.

### Generic runtime

`pitgun-runtime` owns domain-neutral deterministic execution mechanisms:

- stable seeded random streams and their compatibility implementations;
- deterministic execution context and logical ordering;
- workload identity and model/version binding;
- run and receipt verification orchestration;
- comparison-profile dispatch;
- domain verifier hooks;
- execution errors that do not encode domain concepts.

It does not own:

- Racing, Grid, or other domain equations;
- circuits, vehicles, laps, power buses, or domain state;
- filesystem persistence, network transport, or CLI presentation;
- a universal numerical solving algorithm;
- a dynamic plugin ABI in the first implementation.

Versioned wire shapes, canonical JSON, digests, Run Bundle schemas, telemetry
frames, and comparison-profile identifiers remain in `pitgun-contract`.
Filesystem Run Bundle persistence remains an application adapter. Telemetry
processing remains in `pitgun-core`.

### Domain Solver

A domain Solver computes a physical or mathematical solution for a defined
class of models. `pitgun-racing-solver` owns:

- track and vehicle solution inputs;
- velocity, braking, acceleration, energy, and integration algorithms;
- deterministic physical solution types;
- Solver-level tests and numerical invariants.

It does not orchestrate championships, sessions, strategies, pit events, or
leaderboards.

### Domain Simulator

A domain Simulator evolves application state through logical time and invokes
its Solver when a physical solution is required.
`pitgun-racing-simulator` owns:

- race, session, lap, competitor, strategy, and event orchestration;
- Racing data-pack resolution;
- Racing telemetry production;
- the domain workload adapter used by `pitgun-runtime`;
- the browser-facing WebAssembly facade for the complete Racing workload.

The dependency `pitgun-racing-simulator → pitgun-racing-solver` is intentional.
The reverse dependency is forbidden.

### Domain contracts and policies

`pitgun-racing-contract` owns Racing input, output, evidence, catalog, and
payload schemas that must cross process or repository boundaries. It builds on
generic identifiers and telemetry types without adding Racing fields to
`pitgun-contract`.

`pitgun-racing-policy` owns Racing-specific canonicalization and validation. It
uses the generic policy engine rather than embedding `RaceInput`, vehicle, or
setup rules inside `pitgun-policy`.

## Dependency Direction

An arrow means “may depend on”:

```text
pitgun-core ───────────────→ pitgun-contract
pitgun-runtime ────────────→ pitgun-contract
pitgun-policy ─────────────→ pitgun-contract
pitgun-signing ────────────→ pitgun-contract

pitgun-racing-contract ────→ pitgun-contract
pitgun-racing-solver ──────→ pitgun-racing-contract
pitgun-racing-solver ──────→ pitgun-runtime
pitgun-racing-policy ──────→ pitgun-racing-contract
pitgun-racing-policy ──────→ pitgun-policy
pitgun-racing-simulator ───→ pitgun-racing-contract
pitgun-racing-simulator ───→ pitgun-racing-solver
pitgun-racing-simulator ───→ pitgun-racing-policy
pitgun-racing-simulator ───→ pitgun-runtime

pitgun-cli ────────────────→ pitgun-runtime
pitgun-cli ────────────────→ pitgun-core
pitgun-cli ────────────────→ pitgun-racing-simulator
```

Generic crates must never depend on `pitgun-racing-*` crates. Domain crates may
depend on generic crates.

## Workload Integration

The first runtime interface supports workloads linked at Rust compile time. A
workload adapter binds its model identity, accepted input, execution logic,
output, telemetry, and verification hooks to the runtime. The exact Rust trait
surface is defined by the runtime implementation ticket, not by this ADR.

The existing browser module remains valid: Rust domain crates are compiled into
one WebAssembly artifact with the game. This is static linkage targeting WASM,
not a user-supplied plugin loaded after deployment.

Loading arbitrary external `.wasm` workloads is deferred. It requires a
versioned component ABI, sandbox capabilities, resource limits, artifact trust,
and an upgrade policy. Those constraints must be designed independently before
the hosted platform accepts third-party code.

## Compatibility Profiles

The runtime uses the profiles already defined by the deterministic run
contract:

- `portable-exact-v1` requires byte-identical canonical output and telemetry
  evidence across supported native and WASM runtimes;
- `bounded-float-v1` permits only explicitly declared, versioned tolerance
  comparisons.

Racing remains the `portable-exact-v1` conformance workload. The runtime must
fail closed on `bounded-float-v1` until its comparison-manifest type and verifier
are implemented and covered by cross-runtime fixtures.

## Genericity Rule

Pitgun generalizes the execution lifecycle, not every domain algorithm. A
Solver or Simulator abstraction is promoted into the generic runtime only after
at least Racing and one materially different second domain demonstrate the same
semantic need.

Similar naming or method signatures are not sufficient evidence of a reusable
abstraction.

## Migration Map

| Current ownership | Target ownership | Delivery |
|---|---|---|
| `pitgun-solver::rng` | `pitgun-runtime` | #83 |
| `pitgun-solver::kernel` physical solution code | `pitgun-racing-solver` | Revised #39 |
| Race/session orchestration in `pitgun-solver` | `pitgun-racing-simulator` | Revised #39 |
| Racing WASM facade | `pitgun-racing-simulator` | Revised #39 and coordinated game PR |
| Racing schemas in `pitgun-contract` | `pitgun-racing-contract` | Revised #42 |
| Racing validation in `pitgun-policy` | `pitgun-racing-policy` | Revised #43 |
| Generic replay and verification logic in `pitgun-cli` | `pitgun-runtime` plus CLI filesystem adapter | #83 |
| Racing golden workload under `pitgun-solver` | Simulator-level conformance test, with Solver fixtures kept separately | Revised #39 |

Every migration must preserve the published seed-42 `run_id`, canonical output,
telemetry evidence, Run Bundle layout, native/WASM golden vectors, and CLI final
`VERIFIED` line. A crate rename alone never justifies changing logical evidence.

## Consequences

Positive consequences:

- Solver and Simulator retain precise, testable responsibilities;
- generic crates stop importing Racing semantics;
- a future domain can supply different algorithms without pretending to reuse
  Racing physics;
- the runtime becomes the stable platform integration point;
- static Rust and WASM delivery continue without designing an unsafe plugin
  system prematurely.

Costs and trade-offs:

- the workspace gains several explicit domain crates;
- the game and framework repositories require a coordinated WASM migration;
- crate moves must preserve published compatibility vectors;
- some abstractions remain intentionally duplicated until a second domain
  proves they are genuinely generic.

## Rejected Alternatives

### Turn the current `pitgun-solver` directly into a universal Solver

Rejected because its algorithms and public types are currently Racing-specific.
Renaming them generic would hide coupling rather than remove it.

### Move every Racing algorithm into one Simulator crate

Rejected because it collapses physical solution and time orchestration into one
ownership boundary and contradicts the intended Solver/Simulator separation.

### Define a universal Solver trait before implementing a second domain

Rejected for now because a trait derived only from Racing would encode Racing
assumptions as framework requirements. The linked workload boundary is the
smaller and more durable first abstraction.

### Accept user-provided WASM models immediately

Rejected for the first runtime version because dynamic execution introduces ABI,
sandbox, resource-governance, and trust requirements unrelated to the crate
boundary cleanup.

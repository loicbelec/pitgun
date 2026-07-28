# ADR 0002: Optional Versioned Catalogs and Resolved Scenarios

- Status: Accepted
- Date: 2026-07-28
- Decision issue: [#128](https://github.com/loicbelec/pitgun/issues/128)
- Parent epic: [#123](https://github.com/loicbelec/pitgun/issues/123)

## Context

Pitgun models need stable executable inputs, while applications need to discover
and present domain resources.

The Racing reference workload currently compiles circuits, vehicles, engines,
tires, drivers, aero definitions, and chassis definitions into the Rust/WASM
artifact. The browser consumes catalog-shaped exports from that artifact for
both simulation and user-interface choices. Adding a data-only Racing resource
therefore requires editing Rust source tables, recompiling the WASM package, and
redeploying the game.

That ownership is useful as an offline fallback, but it is not a sustainable
publication boundary. A future Grid workload may similarly need network
topologies, generator definitions, and demand profiles. Other workloads may
require no externally published resource catalog at all.

Pitgun already has two related but distinct concepts:

- `DeterministicRunContractV1` binds a versioned and content-addressed
  `data_pack` into logical `run_id`;
- `schemas.pitgun.io` publishes executable wire schemas through a public schema
  registry that existing documentation sometimes calls a catalog.

Adding another undifferentiated `catalog_ref` directly to the deterministic
contract would duplicate `data_pack`, make presentation-only changes alter
logical execution identity, and create ambiguous ownership.

Pitgun needs a catalog boundary that supports applications and hosted
verification without making HTTP, OVH, or network availability a requirement
of the deterministic framework.

## Decision

Pitgun distinguishes an optional **Resource Catalog** from the deterministic
**Simulation Pack** already bound by a run contract.

The catalog is a discovery, distribution, and presentation mechanism. A model
may use one, but the generic runtime does not require one.

The simulation pack is the canonical execution dependency selected from or
embedded by a workload. Its `ArtifactIdentity` remains part of deterministic
run identity.

### Vocabulary

#### Model Artifact

A `ModelArtifact` identifies the versioned executable semantics of one domain
model. The existing contract `model: ArtifactIdentity` remains its V1 wire
representation.

Examples include:

- the Racing physical and orchestration workload;
- a future electrical load-flow workload;
- a catalog-free constant or mathematical workload.

#### Resource Catalog

A Resource Catalog is an application-facing, immutable release that groups
domain resources and their presentation metadata.

A catalog release manifest identifies:

- the catalog ID and release version;
- its simulation pack;
- its presentation pack;
- the canonical digest of each pack;
- the resource indexes and immutable relative paths needed to retrieve them;
- compatibility requirements for models and contract schemas.

The complete release manifest has its own digest for publication and cache
integrity. That full digest is not automatically part of deterministic
`run_id`.

#### Simulation Pack

A Simulation Pack contains only data that can influence model execution. It is
versioned and content-addressed as an `ArtifactIdentity`.

For `DeterministicRunContractV1`, the resolved simulation pack is bound through
the existing `data_pack` field. V1 is not changed and its published vectors
remain exact.

If a future contract version changes or removes the `data_pack` requirement,
that requires a separate contract decision. Catalog optionality does not itself
authorize such a wire change.

#### Presentation Pack

A Presentation Pack contains labels, descriptions, images, colors, grouping,
and other application metadata that cannot influence execution.

It has an independent identity and digest. Changing presentation data may
create a new catalog release, but it must not change the simulation pack
identity or deterministic `run_id`.

If a field is used by both execution and presentation, its execution meaning
belongs to the Simulation Pack. The presentation layer may derive a view from
it, but it must not maintain a conflicting second definition.

#### Scenario

A Scenario describes what a user or application asks the model to execute. It
selects resource identifiers and provides configuration values, initial
conditions, strategy, seed-independent inputs, and other domain parameters.

The existing `ScenarioIdentity` identifies the versioned scenario semantics.
The canonical input artifact contains the concrete scenario values and is
already bound by `input.digest`.

#### Resolved Scenario

A `ResolvedScenario` is the immutable, validated in-memory result of combining:

- a model identity;
- a scenario and its canonical input;
- the selected simulation pack;
- every referenced domain resource.

Resolution must finish before execution begins. A missing resource,
incompatible version, invalid digest, or invalid domain reference is an error;
the runtime must not start a partial simulation.

`ResolvedScenario` is an architectural boundary, not necessarily a new public
wire artifact in V1. The contract remains the durable identity record.

#### Run Contract and Verification Proof

The Run Contract fixes logical execution semantics. `run_id` remains the digest
of its complete canonical bytes.

A Verification Proof or verdict binds:

- the recomputed `run_id`;
- the concrete execution receipt;
- the model and simulation pack actually resolved;
- output and telemetry evidence;
- the verifier identity and structured decision.

It does not trust a client-provided catalog, digest, or verdict.

### Catalog and contract relationship

The relationship is:

```text
Catalog discovery
        │
        ▼
Immutable catalog release
        │
        ├── Presentation Pack ──► application UI
        │
        └── Simulation Pack
                 │
Scenario ────────┤
                 ▼
         Resolved Scenario
                 │
                 ▼
       Deterministic Run Contract
       model + data_pack + input
```

The mutable name `latest` is only a discovery pointer. An application may read
it before choosing a release, but neither `latest` nor its URL may appear as the
reproducibility identity of a signed run.

Once selected, an execution pins immutable identities and digests. A complete
Racing weekend keeps the same selected release even if `latest` advances.

### Catalog-free models

A workload that does not use a Resource Catalog bypasses catalog discovery and
catalog resolution entirely.

Under the existing V1 run contract, the workload still supplies the required
`data_pack: ArtifactIdentity` according to its own static execution semantics.
That artifact is not a placeholder Resource Catalog, and the runtime does not
instantiate a catalog service for it.

The distinction is:

```text
Catalog-backed workload:
  Resource Catalog → Simulation Pack → data_pack binding

Catalog-free workload:
  Workload-owned Simulation Pack ────→ data_pack binding
```

The generic execution lifecycle remains valid in both cases.

## Loading and Resolution Boundary

Pitgun standardizes validated values and pure resolution behavior, not one
mandatory I/O implementation.

The deterministic core accepts bytes or an already validated snapshot. It does
not:

- perform HTTP requests;
- read OVH or SFTP configuration;
- hard-code `catalog.pitgun.io`;
- require credentials or network access;
- select `latest` during execution;
- retry a mutable remote dependency from inside a simulation.

Applications own transport adapters:

```text
Browser   fetch/cache bytes ───────► validate snapshot
CLI       filesystem bytes ────────► validate snapshot
Authority HTTP/cache bytes ────────► validate snapshot
Verifier  archive/cache bytes ─────► validate snapshot
Tests     fixture/embedded bytes ──► validate snapshot
```

The first implementation should prefer pure parsing, validation, and resolution
functions over a universal asynchronous loader trait. A reusable loader trait
may be introduced later only if multiple materially different domains prove the
same interface.

`CatalogResolver` therefore names an architectural responsibility, not a
required deployable service.

## Ownership

Generic ownership:

- `pitgun-contract` owns domain-neutral identities, canonical manifest
  primitives, and digest rules;
- `pitgun-runtime` consumes a workload that has already resolved valid
  execution inputs;
- generic crates never import Racing catalog schemas.

Racing ownership:

- `pitgun-racing-contract` owns Racing resource and manifest schemas;
- `pitgun-racing-simulator` owns Racing pack validation, resource lookup,
  scenario resolution, and the compatibility facade used by the browser;
- `pitgun-racing-solver` receives resolved physical inputs and never performs
  catalog discovery;
- the game owns browser fetch, cache, release selection, and presentation;
- the authority owns allowed-release policy and signed contract issuance;
- the verifier owns independent retrieval and digest validation for replay.

The first Racing release lives in the framework repository as reference
workload data. A separate catalog repository may be introduced later if release
cadence or ownership justifies it.

## Publication

Catalog V1 is published as static files at `catalog.pitgun.io` on OVH shared
hosting.

Example:

```text
catalog.pitgun.io/
└── racing/
    ├── latest.json
    └── v1.0.0/
        ├── catalog.json
        ├── simulation/
        ├── presentation/
        └── assets/
```

Versioned paths are immutable and receive long-lived immutable caching.
`latest.json` receives short-lived or no-cache behavior. Publication validates
schemas and digests before SFTP upload and verifies the public bytes after
deployment.

Static publication requires no PHP, database, or VPS service. An administration
API, private catalog registry, or generated catalog service is a separate future
capability.

## Availability and Failure Semantics

- A cached immutable release remains usable if discovery is unavailable.
- The Racing application retains an embedded fallback generated from a known
  catalog release for offline and recovery use.
- A release selected for an in-progress execution or weekend never changes.
- A missing or corrupt presentation pack may degrade application presentation,
  but it cannot silently substitute simulation data.
- A missing, incompatible, or digest-invalid simulation pack prevents hosted
  deterministic execution.
- Transient retrieval failure in the hosted verifier produces `PENDING`, not
  `REJECTED`.
- An unknown identity or confirmed digest mismatch fails closed.
- Historical simulation packs referenced by accepted contracts remain
  retrievable for the declared retention period.

## Security and Proprietary Content

Any catalog content downloaded by a browser must be considered public. WASM,
obfuscation, signed URLs, or client-side encryption do not make simulation data
proprietary after delivery.

Future proprietary models or resource packs require an authorization boundary
and usually server-side execution. They must not weaken the public deterministic
contract or the open Racing reference workload.

Catalog signatures may later authenticate a publisher. V1 SHA-256 digests
provide content identity and integrity when bound by a signed authority
contract; they do not independently establish publisher trust.

## Compatibility and Migration

The migration is incremental:

1. Define catalog manifest and resolution contracts without modifying
   `DeterministicRunContractV1`.
2. Extract the current Racing data into one immutable release.
3. Generate the embedded fallback from that same release.
4. Preserve current native, WASM, CLI, browser IDs, and golden evidence.
5. Publish the release statically.
6. Migrate the game UI and WASM consumer to one pinned release on staging.
7. Retire transitional crate, data, and fixture ownership only after every
   consumer has moved.
8. Bind the selected simulation pack in authority issuance and verifier replay.

The delivery is tracked by [#123](https://github.com/loicbelec/pitgun/issues/123).

## Consequences

Positive consequences:

- new data-only Racing resources can be published without recompiling the game
  or WASM;
- the UI, native runtime, browser runtime, authority, and verifier can use one
  release;
- deterministic identity remains independent of labels and images;
- the framework remains useful for models with no Resource Catalog;
- static hosting is independent of VPS availability;
- historical replay binds immutable content rather than mutable URLs.

Costs and trade-offs:

- publication introduces manifest, schema, digest, cache, and retention rules;
- the application must handle discovery, caching, and explicit failure states;
- the authority and verifier need an archive or cache of accepted simulation
  packs;
- changing simulation content intentionally changes `data_pack` identity and
  therefore `run_id`;
- the embedded fallback must be generated and audited rather than edited
  independently.

## Rejected Alternatives

### Add the full catalog release digest directly to V1 `run_id`

Rejected because the release includes presentation data. Correcting a label or
image would then change logical execution identity despite identical simulation
semantics. V1 already binds the correct simulation dependency through
`data_pack`.

### Replace `data_pack` with `catalog_ref` in V1

Rejected because it breaks the accepted contract and published compatibility
vectors while conflating discovery with execution identity.

### Make HTTP loading part of `pitgun-runtime`

Rejected because deterministic execution must not depend on a transport,
hostname, provider, credential, mutable pointer, or network availability.

### Require every model to implement a catalog

Rejected because catalogs are a domain/application capability, not a universal
property of deterministic computation.

### Build a PHP or VPS catalog API for V1

Rejected because immutable static JSON and assets satisfy discovery,
distribution, caching, and replay requirements with less operational risk.

### Maintain separate UI and simulation catalogs

Rejected because duplicated identities drift. One release may expose separate
simulation and presentation packs, but it owns their relationship and validates
all cross-references.

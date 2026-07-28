# Resource Catalog Contract V1

Resource Catalog V1 is the optional, domain-neutral contract used to discover
and distribute immutable model resources. It does not replace the
[Deterministic Run Contract V1](DETERMINISTIC_RUN_CONTRACT_V1.md).

The Rust implementation is owned by `pitgun-contract`. Its public JSON Schemas
are:

- `https://schemas.pitgun.io/resource-catalog/v1.json`;
- `https://schemas.pitgun.io/catalog-release-identity/v1.json`.

## Identity model

A release manifest contains stable catalog coordinates, one Simulation Pack,
one Presentation Pack, and a closed compatibility declaration.

```text
Resource Catalog release
├── Simulation Pack ────► DeterministicRunContractV1.data_pack
├── Presentation Pack ──► application UI only
└── Compatibility ──────► exact contract and model versions
```

Each pack is identified by the existing `ArtifactIdentity`:

```text
id + exact SemVer + sha256 digest
```

The digest of a pack identity must equal the digest of its canonical resource
index. The release root and transport URL are deliberately absent from the
manifest. All index paths are safe, lowercase, POSIX-style relative paths.

The complete manifest digest is carried by a separate
`CatalogReleaseIdentityV1`. Keeping the identity external avoids a circular
self-digest inside the manifest.

Domain-specific resource indexes can reuse `CatalogResourceV1` for each
content-addressed file: stable resource ID, immutable relative path, exact
lowercase media type, and digest of the stored bytes. Racing- or Grid-specific
fields remain in their respective domain contracts.

## Deterministic boundary

Before starting a catalog-backed run, a consumer must:

1. parse the manifest using its exact supported schema version;
2. validate canonical ordering and pack-index digests;
3. recompute and verify the external release identity;
4. verify the exact deterministic contract and model versions;
5. verify that `simulation_pack.identity` equals the run contract's
   `data_pack`;
6. resolve every domain resource referenced by the scenario;
7. pass the fully resolved values to the workload.

The URL used to retrieve the release, a mutable `latest` pointer, and the
Presentation Pack never enter `run_id`.

Changing simulation content creates a new Simulation Pack identity and
therefore a different V1 `data_pack` binding. Changing only presentation data
may create a new catalog release, but does not alter the Simulation Pack or
`run_id`.

## Compatibility

V1 compatibility is intentionally closed:

- schema and compatibility versions are exact enumerations;
- deterministic contract versions are exact, not ranges;
- model identifiers and versions are declared explicitly;
- unknown schema, compatibility, contract, or model versions fail closed.

The compatible-model array is ordered strictly by model identifier. Each
model's version set is non-empty and serializes in semantic-version order.
These rules produce stable canonical manifest bytes across runtimes.

## Catalog-free workloads

Resource Catalogs remain optional. A catalog-free workload constructs a
`ResolvedScenario` directly and continues to bind its workload-owned
Simulation Pack through the required V1 `data_pack`.

It does not invent a placeholder catalog, perform discovery, or depend on a
catalog service.

## Resolved scenarios

`ResolvedScenario<Input, Resources>` is a validated Rust boundary, not a new
V1 wire artifact. The generic contract owns:

- the durable `DeterministicRunContractV1`;
- the optional catalog release identity;
- the resolved domain input;
- the resolved domain resources.

Concrete input and resource types remain owned by Racing, Grid, or another
domain. `pitgun-contract` never imports those domain schemas.

## Loading and publication

The contract contains no HTTP, OVH, filesystem, cache, or retry behavior.
Applications retrieve bytes and then invoke pure parsing, validation, and
resolution:

```text
Browser / CLI / Authority / Verifier
                  │
                  ▼
          immutable bytes
                  │
                  ▼
       validate and resolve
                  │
                  ▼
         ResolvedScenario
```

A V1 catalog can therefore be published as static JSON and assets while the
same contracts remain usable from embedded fixtures, local files, archives, or
an independently cached verifier.

The architectural rationale and operational policy are fixed by
[ADR 0002](adr/0002-optional-versioned-catalogs.md).

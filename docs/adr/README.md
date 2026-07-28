# Architecture Decision Records

Architecture Decision Records capture durable Pitgun decisions that constrain
crate ownership, public contracts, and migration work.

| ADR | Status | Decision |
|---|---|---|
| [0001](0001-runtime-and-domain-workloads.md) | Accepted | Separate the generic deterministic runtime from domain-specific Solver and Simulator crates |
| [0002](0002-optional-versioned-catalogs.md) | Accepted | Keep Resource Catalogs optional, bind simulation packs rather than presentation to deterministic identity, and keep loading outside the core |

An accepted ADR is changed only to correct factual errors or clarify wording.
A decision change requires a new ADR that supersedes the previous record.

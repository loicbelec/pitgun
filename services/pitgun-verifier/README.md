# pitgun-verifier

`pitgun-verifier` independently validates deterministic execution evidence. The
Racing workload is its first hosted adapter.

The verification engine:

1. verifies the Authority signature, audience, validity window, and `run_id`;
2. checks the strict Racing V1 contract profile;
3. resolves the statically linked model and retained immutable Simulation Pack;
4. checks the retained Authority policy identity;
5. recalculates receipt, output, and telemetry-summary digests;
6. binds the concrete `execution_id` to the authorized run;
7. replays the exact Racing workload through `pitgun-runtime`;
8. emits `VerificationVerdictV1`.

## Catalog-backed and direct-pack runs

The signed deterministic contract contains the immutable Simulation Pack
identity, not the mutable catalog discovery path. A catalog-backed Authority
request and a direct immutable-pack request therefore have the same verifier
semantics when they select the same pack.

The verifier resolves that exact identity from its retained catalog/archive.
Missing retained bytes produce a retryable `PENDING` decision. A present but
different identity produces terminal `REJECTED`.

## Boundaries

This crate currently contains transport-independent verification logic. It does
not:

- trust a client-provided verdict;
- persist leaderboard rows;
- consume an authorization nonce;
- claim durable idempotency;
- perform mutable `latest` discovery;
- expose an HTTP API.

Those responsibilities belong to the hosted API and persistence increments
around this engine. Keeping the engine pure also allows the same verifier to
move from the VPS to another worker, including a future Mac mini, without
changing the verdict contract.

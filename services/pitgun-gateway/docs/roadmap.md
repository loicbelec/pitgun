# pitgun-gateway Roadmap

## Current baseline

The gateway is the hosted ingress boundary for versioned Pitgun event
envelopes. Its current executable baseline includes:

- authenticated WebSocket ingestion;
- schema, timestamp, payload-size, and rate validation;
- idempotent event and lap-summary persistence in PostgreSQL;
- optional telemetry and summary projection into QuestDB;
- optional run-configuration mirroring to the game performance API;
- health and Prometheus metrics endpoints;
- deterministic summary construction from the canonical `sim.*` dictionary.

PostgreSQL is required through `PITGUN_GATEWAY_DATABASE_URL` or `DATABASE_URL`.
QuestDB and run-registry integrations are optional and enabled through their
dedicated URLs. SQLite is not a supported gateway backend.

## Near-term hardening

- Make the public envelope schema and the Rust parser reject the same inputs.
- Add explicit acknowledgement semantics for clients that require delivery
  confirmation.
- Add dead-letter evidence for rejected events without storing credentials or
  unbounded payloads.
- Define retention and replay policies for PostgreSQL and QuestDB.
- Exercise staging deployment and rollback through `loicbelec/infra-vps`.

## Later platform work

- Verify deterministic run receipts against registered contracts.
- Apply per-model and per-tenant quotas at the hosted boundary.
- Dispatch accepted jobs to heterogeneous runners without coupling the gateway
  to Racing.
- Expose auditable execution status and evidence through versioned APIs.

## Deployment ownership

This repository builds and publishes the gateway image. The canonical staging
and production Compose stacks, routing, secrets wiring, persistence volumes,
observability, and deployment workflows live in `loicbelec/infra-vps`.

# pitgun-gateway Roadmap

## Current baseline

The gateway is the hosted ingress boundary for versioned Pitgun event
envelopes. It provides:

- authenticated WebSocket ingestion;
- schema, timestamp, payload-size, and rate validation;
- idempotent append-only event persistence in PostgreSQL;
- health and Prometheus metrics endpoints.

PostgreSQL is required through `PITGUN_GATEWAY_DATABASE_URL` or `DATABASE_URL`.
SQLite is not supported. Racing summaries, analytical projections, LLM calls,
and run-registry mirroring are intentionally outside this service.

## Near-term hardening

- Make the public envelope schema and the Rust parser reject the same inputs.
- Add explicit acknowledgement semantics for clients that require delivery confirmation.
- Add dead-letter evidence for rejected events without storing credentials or unbounded payloads.
- Define retention and replay policies for the PostgreSQL event log.
- Exercise staging deployment and rollback through `loicbelec/infra-vps`.

## Later platform work

- Define a versioned boundary between generic accepted events and domain-owned analytics.
- Verify deterministic run receipts against registered contracts.
- Apply per-model and per-tenant quotas at the hosted boundary.
- Dispatch accepted jobs to heterogeneous runners without coupling the gateway to Racing.
- Expose auditable execution status and evidence through versioned APIs.

## Deployment ownership

This repository builds and publishes the gateway image. The canonical staging
and production Compose stacks, routing, secrets wiring, persistence volumes,
observability, and deployment workflows live in `loicbelec/infra-vps`.

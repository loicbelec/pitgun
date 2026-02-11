# pitgun-gateway Roadmap (MVP -> v1 -> v1.1)

## Scope and source of truth
Telemetry payloads are driven by existing Pitgun contract types:
- `pitgun-contract::TelemetryFrame`
- `pitgun-contract::Sample`
- `pitgun-contract::SampleValue`
- `pitgun-contract::Event`
- `pitgun-contract::EventData`
- `pitgun-contract::SignalQuality`
- `pitgun-contract::Parameter` / `pitgun-contract::ParameterRegistry` (channel dictionary)

`telemetry.sample_batch` in the gateway is a thin wrapper around `Vec<TelemetryFrame>`.
No custom telemetry schema is introduced in the gateway.

## MVP (ship fast, stable)
### Deliverables
- WebSocket ingestion endpoint (`/ws`) with API-key auth.
- Event envelope validation:
  - `schema_version`
  - `event_id` (UUID)
  - `ts` (ISO8601/RFC3339)
  - `player_id`
  - `session_id`
  - `event_type`
  - `payload`
- Required event types:
  - `session.start`
  - `telemetry.sample_batch`
  - `session.end`
  - `purchase.order_completed`
- Limits and protection:
  - max payload size per message
  - max messages/sec per connection
- Durable append-only persistence in SQLite:
  - `events` table
  - idempotence by unique `event_id`
  - indexes on `(session_id, ts)`, `(player_id, ts)`, `(event_type, ts)`
- `/health` endpoint checks DB readiness.
- Docs and examples for local testing with `wscat` / `websocat`.

### Acceptance criteria
- Valid event sent over `/ws` is persisted once, even when replayed with same `event_id`.
- Invalid schema/timestamp/payload is rejected.
- Connection exceeding message rate limit is closed with policy error.
- Gateway restart preserves previously ingested events.
- `/health` returns `200` when DB is reachable.

## v1 (production hardening)
### Deliverables
- Optional Postgres backend (config switch) while keeping the same envelope/event model.
- Structured ingress metrics (accepted, rejected, duplicate, queue-full, validation errors).
- Dead-letter output for invalid events (file sink or table) for debugging.
- Replay tooling (`SELECT` templates and/or CLI command) by `session_id` and time range.

### Acceptance criteria
- Service can run with SQLite or Postgres without event model changes.
- Metrics allow identifying auth failures, invalid payloads, and rate-limit drops.
- Replay by session returns deterministic event ordering by `ts` then insertion order.

## v1.1 (operability)
### Deliverables
- Optional acknowledgement protocol over WS (`accepted`/`duplicate`/`error`) for clients that need delivery feedback.
- Backpressure signaling and per-player/session quotas.
- Event retention policy and compaction strategy.
- Signed ingest contracts integration (authority-issued ingest constraints).

### Acceptance criteria
- Clients can optionally receive explicit ack/nack without breaking fire-and-forget mode.
- Operational limits configurable per environment and observable in logs/metrics.
- Retention policy can prune data without corrupting replay ability for retained windows.

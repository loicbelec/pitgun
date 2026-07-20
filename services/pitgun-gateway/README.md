# pitgun-gateway

Hosted WebSocket ingestion boundary for versioned Pitgun events.

- Transport: `ws://` (behind the reverse proxy: `wss://telemetry.pitgun.com`)
- Health: `GET /health`
- Metrics: `GET /metrics` (Prometheus text format)
- Ingest: `GET /ws` (JSON text messages)
- Storage: PostgreSQL append-only `events` table, idempotent on `event_id`

The gateway deliberately does not build Racing summaries, project telemetry into
an analytical database, call an LLM, or dispatch domain jobs. Domain-specific
analytics and execution belong behind separate versioned interfaces.

## Pitgun-native telemetry model

`telemetry.sample_batch` reuses `TelemetryFrame`, `Sample`, `SampleValue`,
`Event`, `EventData`, and `SignalQuality` from `pitgun-contract`. Its
payload is a thin `frames: Vec<TelemetryFrame>` wrapper.

Public envelope schema:

- `https://schemas.pitgun.io/pitgun-envelope/v1.json`

Source schema:

- `schemas/pitgun-envelope/v1.json`

See `docs/event-model.md` for details.

## Event envelope

Every message over `/ws` must follow:

```json
{
  "schema_version": "pitgun-envelope-v1",
  "event_id": "UUID",
  "ts": "ISO8601",
  "player_id": "string",
  "session_id": "string",
  "event_type": "session.start | telemetry.sample_batch | session.end | purchase.order_completed | pitwall.session_configured",
  "payload": {}
}
```

`pitwall.session_configured` remains accepted by envelope v1 for compatibility,
but the gateway treats it as an ordinary persisted event and does not mirror it
to a Racing service.

## Validation, authentication, and limits

- API key via `x-api-key`, `Authorization: Bearer <token>`, or the
  `?token=...` / `?api_key=...` query string for browser WebSocket clients.
- Required fields, schema version, UUID, timestamp, event type, and payload
  shape are validated before queueing.
- Message size and per-connection rate are bounded.
- Duplicate `event_id` values are ignored by a unique PostgreSQL constraint.

## Metrics

`GET /metrics` exposes:

- `pitgun_gateway_ws_messages_total`
- `pitgun_gateway_ws_message_bytes_total`
- `pitgun_gateway_events_ingested_total{event_type}`
- `pitgun_gateway_events_rejected_total{reason}`
- `pitgun_gateway_postgres_writes_total{outcome}`
- `pitgun_gateway_parse_latency_seconds_count`
- `pitgun_gateway_parse_latency_seconds_sum`
- `pitgun_gateway_postgres_write_latency_seconds_count`
- `pitgun_gateway_postgres_write_latency_seconds_sum`

## PostgreSQL storage

The append-only `events` table stores the event identity and routing fields,
the typed payload as JSON, the full original envelope for replay/debug, receipt
time, remote IP, and user agent. It is indexed by session, player, weekend, and
event type with their event timestamps.

The gateway creates no derived lap, session, practice, race, or insight tables.
An existing deployment may retain old tables until its dedicated data-retirement
operation; the current executable neither reads nor writes them.

## Environment variables

- `PITGUN_GATEWAY_BIND` (default `127.0.0.1:8080`)
- `PITGUN_GATEWAY_ALLOW_NON_LOOPBACK` (default disabled)
- `PITGUN_GATEWAY_DATABASE_URL` (required unless `DATABASE_URL` is set)
- `PITGUN_GATEWAY_SCHEMA_VERSION` (default `pitgun-envelope-v1`)
- `PITGUN_GATEWAY_API_KEY` (single key)
- `PITGUN_GATEWAY_API_KEYS` (comma-separated keys)
- `PITGUN_GATEWAY_MAX_MESSAGE_BYTES` (default `524288`)
- `PITGUN_GATEWAY_MAX_MESSAGES_PER_SEC` (default `120`)
- `PITGUN_GATEWAY_INGEST_QUEUE_SIZE` (default `4096`)
- `RUST_LOG` (default `info`)

## Local run

```bash
PITGUN_GATEWAY_API_KEY=dev-secret \
PITGUN_GATEWAY_BIND=127.0.0.1:8080 \
cargo run -p pitgun-gateway --release
```

## Test with websocat

```bash
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/session.start.json
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/telemetry.sample_batch.json
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/session.end.json
```

Other envelope-v1 compatibility examples live in `examples/`.

## Deployment ownership

This repository builds and publishes the immutable gateway image.
`loicbelec/infra-vps` owns staging and production Compose stacks, routing,
secrets, persistence, observability, deployment, and rollback.

See `docs/roadmap.md` for the next platform boundaries.

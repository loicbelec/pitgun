# pitgun-gateway

WebSocket ingestion service for Pitgun telemetry and game purchase events.

- Transport: `ws://` (behind reverse proxy => `wss://telemetry.pitgun.com`)
- Health: `GET /health`
- Ingest: `GET /ws` (JSON text messages)
- Storage (MVP): SQLite append-only `events` table with idempotence on `event_id`

## Pitgun-native telemetry model
Telemetry payloads reuse existing Pitgun contract types from `pitgun-contract`:
- `TelemetryFrame`
- `Sample`
- `SampleValue`
- `Event`
- `EventData`
- `SignalQuality`

`telemetry.sample_batch` uses a thin wrapper: `payload.frames: Vec<TelemetryFrame>`.

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
  "event_type": "session.start | telemetry.sample_batch | session.end | purchase.order_completed",
  "payload": {}
}
```

## Validation, auth, and limits
- Auth: API key via `x-api-key`, `Authorization: Bearer <token>`, or query string (`?token=...` / `?api_key=...`) for browser WebSocket clients.
- Required fields validated (schema version, UUID, timestamp, event type, payload shape).
- Max payload size per WS message (`PITGUN_GATEWAY_MAX_MESSAGE_BYTES`).
- Per-connection message rate limit (`PITGUN_GATEWAY_MAX_MESSAGES_PER_SEC`).
- Idempotence: duplicate `event_id` is ignored by unique DB constraint.

## Storage schema (SQLite)
Table: `events`
- `event_id` (unique)
- `schema_version`
- `ts`
- `player_id`
- `session_id`
- `event_type`
- `payload_json`
- `envelope_json` (full original envelope for replay/debug)
- `received_at`
- `remote_ip`, `user_agent`

Indexes:
- `(session_id, ts)`
- `(player_id, ts)`
- `(event_type, ts)`

## Environment variables
- `PITGUN_GATEWAY_BIND` (default `127.0.0.1:8080`)
- `PITGUN_GATEWAY_ALLOW_NON_LOOPBACK` (default disabled)
- `PITGUN_GATEWAY_DB_PATH` (default `./telemetry/events.db`)
- `PITGUN_GATEWAY_DATA_DIR` (legacy fallback; used as `<dir>/events.db` if DB path is not set)
- `PITGUN_GATEWAY_SCHEMA_VERSION` (default `pitgun-envelope-v1`)
- `PITGUN_GATEWAY_API_KEY` (single key)
- `PITGUN_GATEWAY_API_KEYS` (comma-separated keys)
- `PITGUN_GATEWAY_MAX_MESSAGE_BYTES` (default `524288`)
- `PITGUN_GATEWAY_MAX_MESSAGES_PER_SEC` (default `120`)
- `PITGUN_GATEWAY_INGEST_QUEUE_SIZE` (default `4096`)
- `RUST_LOG` (default `info`)

## Local run
```bash
cd /Users/loic/Code/pitgun/pitgun
PITGUN_GATEWAY_API_KEY=dev-secret \
PITGUN_GATEWAY_BIND=127.0.0.1:8080 \
cargo run -p pitgun-gateway --release
```

## Test with websocat
```bash
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/session.start.json
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/telemetry.sample_batch.json
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/session.end.json
websocat -H='x-api-key: dev-secret' ws://127.0.0.1:8080/ws < services/pitgun-gateway/examples/purchase.order_completed.json
```

## Test with wscat
```bash
wscat -c ws://127.0.0.1:8080/ws -H "x-api-key: dev-secret"
```
Paste any example JSON from `examples/`.

## CI/CD (GitHub Actions)
Example workflow: `.github/workflows/build-gateway.yml`

Behavior:
- Build Docker image from workspace Dockerfile (`BIN_NAME=pitgun-gateway`)
- Tag image as `ghcr.io/<org>/pitgun-gateway:<git_sha>`
- Push image to GHCR
- Expose image tag + digest for downstream deployment automation

Recommended repository secrets:
- none required for default GHCR push (uses `GITHUB_TOKEN`)

Recommended deployment model:
- app repo (`pitgun`) builds and pushes immutable images
- infra repo updates compose with digest-pinned images and performs VPS deployment

## Milestones and acceptance criteria
See `docs/roadmap.md`.

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

### Sim-only insight ingress (MVP)

For the insights pipeline, gateway currently evaluates telemetry with a strict
`sim.*` dictionary:

- parameter IDs `5000..5016` only
- mapped to canonical metric keys (for `pitgun-insight-request-v1`)
- non-`sim.*` parameter IDs are ignored for insights
- bad quality/non-numeric samples are dropped for insights
- stats (`min`, `max`, `mean`, `stddev`, etc.) are selected by declared manifest
- a `pitgun-insight-request-v1` payload is built and stored per telemetry event
- optional llm-core call (Ollama `/api/generate`) transforms request into `pitgun-insight-response-v1`
- responses are persisted by `trace_id` for replay/debug

Reference dictionary:

- `portal/schemas/metrics-dictionary/sim.v1.json`

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

Table: `insight_requests`
- `trace_id` (unique, aligned with envelope `event_id`)
- `event_id`
- `run_id`
- `session_id`
- `emitted_at_ms`
- `payload_json` (`pitgun-insight-request-v1`)
- `created_at`

Table: `insight_responses`
- `trace_id` (unique, aligned with insight request `trace_id`)
- `run_id`
- `session_id`
- `status`
- `generated_at_ms`
- `latency_ms`
- `source_model`
- `payload_json` (`pitgun-insight-response-v1`)
- `raw_model_response` (raw JSON text produced by model)
- `created_at`

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
- `PITGUN_GATEWAY_INSIGHT_MANIFEST` (optional path to pipeline manifest for insight stats targets/metrics)
- `PITGUN_GATEWAY_LLM_PROVIDER` (optional, default `ollama`; supported: `ollama`, `openai_compatible`)
- `PITGUN_GATEWAY_LLM_URL` (optional; enables LLM call when set, ex: `http://llm-core:11434/api/generate` for Ollama, or `https://generativelanguage.googleapis.com/v1beta/openai/chat/completions` for Gemini OpenAI-compatible)
- `PITGUN_GATEWAY_LLM_CORE_URL` (legacy alias for `PITGUN_GATEWAY_LLM_URL`)
- `PITGUN_GATEWAY_LLM_API_KEY` (optional bearer token used for `openai_compatible` providers)
- `PITGUN_GATEWAY_LLM_MODEL` (default `llama3.2:3b`, fallback to `OLLAMA_MODEL`)
- `PITGUN_GATEWAY_LLM_TIMEOUT_MS` (default `8000`)
- `PITGUN_GATEWAY_LLM_NUM_CTX` (default `1024`)
- `PITGUN_GATEWAY_LLM_NUM_PREDICT` (default `180`)
- `PITGUN_GATEWAY_LLM_TEMPERATURE` (default `0`)
- `PITGUN_GATEWAY_LLM_DISPATCH_MODE` (default `per_request`; `session_end_summary` sends a single LLM request when `session.end` is received)
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

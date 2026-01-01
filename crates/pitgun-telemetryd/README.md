# pitgun-telemetryd

Telemetry ingestion daemon that sits behind Caddy (`telemetry.pitgun.com` -> `127.0.0.1:8080`). It accepts SessionEnvelope v1 payloads over HTTP and WebSocket, queues them, and writes append-only NDJSON for later processing.

## Endpoints
- `GET /health` – readiness check, returns 200.
- `POST /beacon` – accepts JSON SessionEnvelope v1, returns 202 (fallback for `navigator.sendBeacon`).
- `GET /ws` – WebSocket server; accepts JSON SessionEnvelope v1 text frames (required) and protobuf binary frames (feature `protobuf`).

## Configuration
- `PITGUN_TELEMETRY_BIND` (default `127.0.0.1:8080`) – bind address; must stay on loopback.
- `PITGUN_TELEMETRY_DATA_DIR` (default `/opt/pitgun/telemetry/data`) – NDJSON sink root.
- `RUST_LOG` – tracing filter, e.g. `info,axum=info`.
- Enable protobuf decoding with `--features protobuf` when building.

## Local checks
```bash
cargo build -p pitgun-telemetryd --release

# Health
curl -v http://127.0.0.1:8080/health

# Beacon (JSON, ts_ns as string supported)
curl -X POST http://127.0.0.1:8080/beacon \
  -H "Content-Type: application/json" \
  -d '{
    "schema_version": 1,
    "session_id": "11111111-2222-3333-4444-555555555555",
    "sent_at_ms": 1710000000123,
    "batch": {
      "events": [
        { "channel": "demo", "ts_ns": "1710000000000000000", "value": 1.23 }
      ],
      "end_of_stream": false
    }
  }'
```

## WebSocket test (JSON)
Using websocat:
```bash
websocat ws://127.0.0.1:8080/ws
```
Then paste a batch:
```json
{
  "schema_version": 1,
  "session_id": "11111111-2222-3333-4444-555555555555",
  "batch": {
    "events": [
      { "channel": "demo", "ts_ns": "1710000000000000000", "value": 3.14 }
    ],
    "end_of_stream": false
  }
}
```

## Deployment notes
- Caddy terminates TLS and reverse-proxies to `127.0.0.1:8080`; keep the daemon on localhost only.
- NDJSON files rotate daily under `$PITGUN_TELEMETRY_DATA_DIR/YYYY-MM-DD.ndjson`.
- Install the systemd unit from `deploy/systemd/pitgun-telemetryd.service` (adjust user/group and paths for `/opt/pitgun/pitgun`).

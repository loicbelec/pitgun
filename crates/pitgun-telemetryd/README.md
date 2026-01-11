# pitgun-telemetryd

Telemetry ingestion daemon that sits behind Caddy (`telemetry.pitgun.com` -> `127.0.0.1:8080`). It accepts SessionEnvelope v1 payloads over HTTP and WebSocket, queues them, and writes append-only NDJSON for later processing.

## Endpoints
- `GET /health` – readiness check, returns 200.
- `POST /beacon` – accepts JSON SessionEnvelope v1, returns 202 (fallback for `navigator.sendBeacon`).
- `GET /ws` – WebSocket server; accepts JSON SessionEnvelope v1 text frames (required) and protobuf binary frames (feature `protobuf`).

## Configuration
- `PITGUN_TELEMETRY_BIND` (default `127.0.0.1:8080`) – bind address; must stay on loopback.
- `PITGUN_TELEMETRY_ALLOW_NON_LOOPBACK` (default disabled) – set to `1` or `true` to allow non-loopback bind (e.g., Docker/Traefik); logs a warning and disables the safety guard.
- `PITGUN_TELEMETRY_DATA_DIR` (default `./telemetry/data`) – NDJSON sink root.
  - In Docker: typically set to `/data`
  - On host: e.g. `/opt/volumes/pitgun/telemetryd/data`
- `RUST_LOG` – tracing filter, e.g. `info,axum=info`.
- Enable protobuf decoding with `--features protobuf` when building.

### Local run (non-containerized)
```bash
PITGUN_TELEMETRY_BIND=127.0.0.1:8080 \
cargo run -p pitgun-telemetryd --release
```

### Docker run (behind a reverse proxy)
```bash
docker run \
  -e PITGUN_TELEMETRY_BIND=0.0.0.0:8080 \
  -e PITGUN_TELEMETRY_ALLOW_NON_LOOPBACK=1 \
  -e PITGUN_TELEMETRY_DATA_DIR=/data \
  -p 8080:8080 \
  -v /opt/volumes/pitgun/telemetryd/data:/data \
  pitgun-telemetryd
```

### Docker Compose example
```yaml
services:
  telemetryd:
    image: pitgun-telemetryd
    environment:
      PITGUN_TELEMETRY_BIND: 0.0.0.0:8080
      PITGUN_TELEMETRY_ALLOW_NON_LOOPBACK: "1"
      PITGUN_TELEMETRY_DATA_DIR: /data
    volumes:
      - /opt/volumes/pitgun/telemetryd/data:/data
    ports:
      - "127.0.0.1:8080:8080"
```

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
- A reverse proxy (e.g. Traefik, Caddy) terminates TLS and forwards traffic to the daemon.
- NDJSON files rotate daily under `$PITGUN_TELEMETRY_DATA_DIR/YYYY-MM-DD.ndjson`.
- A systemd unit is provided in `deploy/systemd/pitgun-telemetryd.service` as a reference;
  adjust paths and user/group according to your host layout.
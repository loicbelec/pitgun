# Pitgun-native Event Model

The public JSON Schema for this envelope is:

- `https://schemas.pitgun.io/pitgun-envelope/v1.json`

The schema source lives in `schemas/pitgun-envelope/v1.json`.
Published schema versions must remain backward compatible; incompatible changes require a new version.

Validate schemas and gateway examples from the workspace root with:

```bash
tooling/.venv/bin/python framework/scripts/validate-schemas.py
```

## Envelope (all events)
```json
{
  "schema_version": "pitgun-envelope-v1",
  "event_id": "9a593a28-22f3-48c8-bafe-d3076aad89ad",
  "ts": "2026-02-11T09:00:00Z",
  "player_id": "player-123",
  "session_id": "session-abc",
  "event_type": "telemetry.sample_batch",
  "payload": {}
}
```

## Supported event types
- `session.start`
- `telemetry.sample_batch`
- `session.end`
- `purchase.order_completed`
- `pitwall.session_configured`

## Telemetry payload mapping
`telemetry.sample_batch.payload.frames` is `Vec<pitgun_contract::TelemetryFrame>`.

That means frame internals are reused as-is from Pitgun contract:
- `session_id`, `sequence`, `timestamp_us`, `received_at_us`, `source_id`
- `samples: Vec<Sample>` where `Sample.value` is `SampleValue`
- `events: Vec<Event>`
- motorsport fields (`lap_number`, `sector`, `lap_distance_m`)
- `metadata`

No gateway-only telemetry schema exists.

Channel/parameter semantics remain in `pitgun-contract::registry` (`Parameter`, `ParameterRegistry`),
so `parameter_id` values inside samples can be resolved by Pitgun-native dictionaries.

The JSON Schema mirrors the serialized Rust shape:
- `SampleValue` is represented as `{ "type": "...", "value": ... }`.
- `SignalQuality` and `EventSeverity` use `snake_case` values.
- `metadata` maps string keys to string values.

## Pit Wall session payload

`pitwall.session_configured` remains part of envelope v1 for backward
compatibility. Its payload declares the following Racing run metadata:
- `run_id`
- `track_id`
- `vehicle_id`
- `session_type`
- `seed`
- `sampling_hz`
- `setup`
- `setup_offsets`
- `effective_setup`
- optional build metadata and `stint_strategy`

The gateway validates and persists this event like any other supported
envelope. It does not mirror the payload or interpret its Racing semantics.

## Purchase payload (game-native, PO-like)
`purchase.order_completed.payload` includes:
- `order_id`
- `currency`
- `subtotal`
- `total`
- `tax` (optional)
- `discount` (optional)
- `line_items[]` with `upgrade_id`, `quantity`, `unit_price`, `line_total`
- `purchased_at` (optional RFC3339)

This keeps purchase analytics practical without importing enterprise procurement semantics.

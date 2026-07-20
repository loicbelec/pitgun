# Metrics to Insights Contract (v1)

> Legacy experiment: the current gateway does not produce or consume this
> contract. It is retained as architectural history and may inform a future
> domain-owned analytics service.

This document defines the canonical `metrics -> insights` exchange format between:

- Pitgun Core (deterministic metrics producer)
- LLM service (insight generator)
- Frontend consumers (live insight rendering)

Source schema:

- [`schemas/insight-contract/v1.json`](../schemas/insight-contract/v1.json)
- [`schemas/metrics-dictionary/sim.v1.json`](../schemas/metrics-dictionary/sim.v1.json)

## Design goals

- Deterministic, compact metric inputs
- Structured outputs (not free text blobs)
- Stable versioning and traceability (`run_id`, `session_id`, `trace_id`)
- Clear degraded/error handling paths

## Payload types

The v1 schema supports two payloads:

1. `pitgun-insight-request-v1`
2. `pitgun-insight-response-v1`

## Request (`pitgun-insight-request-v1`)

Required fields:

- `schema_version`
- `run_id`
- `session_id`
- `emitted_at_ms`
- `context`
- `metrics`

`metrics[]` entries are normalized key-value points:

- `key` uses `^[a-z0-9_.-]+$`
- `value` is numeric
- optional metadata: `unit`, `trend`, `horizon`, `confidence`

## Response (`pitgun-insight-response-v1`)

Required fields:

- `schema_version`
- `run_id`
- `session_id`
- `generated_at_ms`
- `status`
- `insights`

`status` enum:

- `ok`
- `degraded`
- `insufficient_data`
- `error`

`insights[]` are fully structured:

- `severity` (`info|advisory|warning|critical`)
- `confidence` (`0..1`)
- `title`
- `rationale`
- `recommendation`
- optional references (`metric_keys`, `tags`, `ttl_ms`)

## Example request

```json
{
  "schema_version": "pitgun-insight-request-v1",
  "run_id": "run_0a1b2c",
  "session_id": "race",
  "trace_id": "trace_abc123",
  "emitted_at_ms": 1773401234567,
  "context": {
    "circuit_id": "MONACO",
    "era": 2026,
    "lap": 31,
    "position": 4,
    "weather": "light_rain",
    "track_status": "green"
  },
  "metrics": [
    { "key": "pace.delta_to_leader_s", "value": 0.45, "unit": "s", "trend": "up", "horizon": "lap" },
    { "key": "tires.front_left.wear_pct", "value": 81.2, "unit": "pct", "trend": "up", "horizon": "stint" },
    { "key": "fuel.delta_laps", "value": -0.3, "unit": "laps", "trend": "down", "horizon": "race" }
  ],
  "constraints": {
    "max_insights": 3,
    "max_words_per_insight": 40,
    "language": "en"
  },
  "policy_version": "policy.v1",
  "prompt_version": "chief-race.v1"
}
```

## Example response

```json
{
  "schema_version": "pitgun-insight-response-v1",
  "run_id": "run_0a1b2c",
  "session_id": "race",
  "trace_id": "trace_abc123",
  "generated_at_ms": 1773401234722,
  "latency_ms": 155,
  "source_model": "llama3-chief-fast:3b",
  "status": "ok",
  "insights": [
    {
      "id": "pit_window_early",
      "severity": "advisory",
      "confidence": 0.78,
      "title": "Open the pit window in 2-3 laps",
      "rationale": "Front-left wear is accelerating while pace delta is rising.",
      "recommendation": "Prepare an early stop for fresh intermediates before traffic peaks.",
      "metric_keys": ["tires.front_left.wear_pct", "pace.delta_to_leader_s"],
      "ttl_ms": 90000,
      "tags": ["strategy", "tires"]
    }
  ]
}
```

## Integration notes

- Keep Core deterministic: only compute metrics, never narrative text.
- Keep LLM output bounded via request `constraints`.
- Always persist request + response by `trace_id` during staged mode.
- UI should render by `severity` and `ttl_ms`, with fallback when status is `degraded` or `error`.

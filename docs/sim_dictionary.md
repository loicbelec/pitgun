# Sim Dictionary (v1)

This document describes the canonical `sim.*` telemetry data dictionary used
by the gateway insight ingress path. The published JSON file is a versioned
registry, not a JSON Schema.

Published dictionary:

- [`schemas/metrics-dictionary/sim.v1.json`](../schemas/metrics-dictionary/sim.v1.json)

## Scope

- Only telemetry samples mapped from `parameter_id` `5000..5016` are accepted
  for the insights ingress path.
- Non-`sim.*` parameters are ignored for insights (while still accepted and
  persisted in raw gateway events).

## Why

- Keep `metrics -> insights` deterministic and low-cardinality in staging.
- Avoid dynamic/non-telemetry channels polluting LLM payloads.
- Establish a stable mapping from simulator channels to canonical metric keys.

## Canonical mapping

Examples:

- `sim.speed_kph` -> `pace.speed_kph`
- `sim.rpm` -> `powertrain.rpm`
- `sim.throttle_pct` -> `driver.throttle_pct`
- `sim.g_lat` -> `dynamics.g_lat`
- `sim.tire_wear_pct` -> `tires.avg_wear_pct`

## Runtime behavior in `pitgun-gateway`

- `telemetry.sample_batch` frames are scanned for mapped `sim.*` parameters.
- Bad quality (`bad` / `no_signal` / `unknown`) and non-numeric values are dropped.
- Accepted event envelopes and lap summaries are persisted in PostgreSQL.
- Canonical telemetry points and generated summaries are projected into QuestDB
  when `PITGUN_GATEWAY_QUESTDB_URL` is configured.
- Summary aggregations are selected by the gateway statistics plan.
- The service logs extraction counters (`sim_points`, dropped counts, unknown IDs)
  and exposes Prometheus metrics for observability.

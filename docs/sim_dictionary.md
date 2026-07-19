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
- Aggregations (`min`, `max`, `mean`, `stddev`, etc.) are selected through a declared
  pipeline manifest (`segment_aggregate.targets[*].metrics`).
- A `pitgun-insight-request-v1` payload is generated from accepted points/stats.
- Insight requests are persisted in SQLite table `insight_requests`.
- The service logs extraction counters (`sim_points`, dropped counts, unknown IDs)
  for observability.

Example manifest:

- [`examples/manifests/pipeline/sim_insight_requests.yaml`](../examples/manifests/pipeline/sim_insight_requests.yaml)

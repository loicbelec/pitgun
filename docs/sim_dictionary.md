# Sim Dictionary (v1)

This document records the canonical `sim.*` telemetry dictionary used by the
retired gateway insight experiment. The published JSON file remains a versioned
registry for compatibility and future Racing analytics; it is not a JSON Schema.

Published dictionary:

- [`schemas/metrics-dictionary/sim.v1.json`](../schemas/metrics-dictionary/sim.v1.json)

## Historical scope

- The former insight projection mapped parameter IDs `5000..5016`.
- Non-`sim.*` parameters were ignored by that projection.
- The current gateway validates and persists the complete event envelope
  without applying this dictionary.

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

## Current runtime behavior

`pitgun-gateway` does not scan channels, construct summaries, or project into
QuestDB. A future Racing-owned analytics component may reuse this dictionary
behind a separate versioned interface.

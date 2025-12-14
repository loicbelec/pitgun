# Segment aggregation (window-by-key)

Pitgun now supports segment/window aggregation driven by a *segment key* channel. The feature is domain-agnostic: the key can represent laps, auction IDs, trade IDs, session IDs, minute buckets, etc.

## Semantics
- The `segment_key` channel is treated as a scalar that must stay constant inside a segment. Any change (increase or decrease) closes the current segment and starts a new one.
- Metrics are computed with Welford’s online algorithm (population stddev). `stddev` is `0.0` for a single sample.
- Non-finite values (NaN/±inf) on either the key channel or any target are skipped with a warning; targets with no samples in a segment return `count=0` and `null` for the other metrics.
- `start_ts_ns`/`end_ts_ns` track the earliest/latest timestamps seen for the segment.
- `emit_on_change` controls whether a segment is emitted immediately when the key changes. `emit_last_segment_on_eof` flushes any open segment when the source marks `end_of_stream` (file-based sources, tests, or controlled replay).

## Minimal manifest snippet
```yaml
- type: segment_aggregate
  segment_key: "segment_id_channel"
  targets:
    - channel: "value_channel"
      metrics: ["mean", "max", "min", "stddev", "count", "sum"]
  emit_on_change: true
  emit_last_segment_on_eof: true
```

## Motorsport example (NLap + nEngine)
`examples/manifests/pipeline/segment_aggregate_engine.yaml` configures:
```yaml
- type: segment_aggregate
  segment_key: "NLap"
  targets:
    - channel: "nEngine"
      metrics: ["count", "min", "max", "mean", "sum", "stddev"]
```

Quick demo with the synthetic replay:
```bash
cargo run -p pitgun-emulator -- \
  --target 127.0.0.1:5001 \
  --input NLap=datasets/synthetic/NLap-demo.csv \
  --input nEngine=datasets/synthetic/nEngine-demo.csv \
  --pace

cargo run -p pitgun-cli -- subscribe \
  --bind 127.0.0.1:5001 \
  --config examples/manifests/pipeline/segment_aggregate_engine.yaml
```
Console output will emit one JSON line per segment with the segment key, start/end timestamps, and the requested metrics.

## HFT-style example
```yaml
- type: segment_aggregate
  segment_key: "auction_id"
  targets:
    - channel: "last_price"
      metrics: ["mean", "max", "min", "stddev", "count"]
    - channel: "size"
      metrics: ["sum", "count"]
  emit_on_change: true
  emit_last_segment_on_eof: true
```

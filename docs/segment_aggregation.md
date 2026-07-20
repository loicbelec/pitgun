# Segment aggregation for observed data

`SegmentAggregateProcessor` groups a stream by a numeric segment-key channel.
The key can represent a lap, operating interval, session, market auction, grid
event, or another domain boundary.

This processor is not part of Pitgun's deterministic execution kernel. It is a
domain-neutral data-engineering primitive that can summarize generated or
captured telemetry. A future comparison workflow can apply the same metrics to
simulation and operation data before calculating their differences.

## Semantics

- The segment key remains constant inside a segment. Any change closes the
  current segment and starts another one.
- Available metrics are `count`, `min`, `max`, `mean`, `sum`, and population
  `stddev`.
- Welford's online algorithm calculates the variance without retaining the
  complete stream.
- Non-finite keys and values are skipped.
- Start and end timestamps track the earliest and latest samples in a segment.
- An end-of-stream marker can flush the final open segment during controlled
  replay.

## Executable example

```bash
cargo run -p pitgun-core --example observed_segment_aggregation
```

The example feeds two laps of observed engine-speed samples through the actual
processor and emits one JSON record per lap. Its implementation lives in the
`pitgun-core` crate so Cargo registers and compiles it with the owning API.

The old UDP manifest demonstration was removed: its emulator and datasets no
longer existed, and the historical manifest format is not a stable Pitgun
contract. The processor and its multi-batch behavior remain covered by unit and
integration tests.

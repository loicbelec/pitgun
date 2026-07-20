# Executable examples

Pitgun keeps only examples that compile and run against the current public
workspace. Historical sketches and prototype manifests remain available in Git
history, but are not presented as supported interfaces.

## Primary example: verified deterministic simulation

Run the complete Racing loop:

```bash
cargo run -p pitgun-cli -- demo racing --seed 42
```

This command executes the versioned scenario, emits typed telemetry, writes an
immutable Run Bundle, reloads it in the verifier, and finishes with
`VERIFIED <run-id>`. It is Pitgun's product-level example and the entry point
described by the [quickstart](QUICKSTART.md).

## Secondary example: observed-data aggregation

Run the standalone telemetry-processing example:

```bash
cargo run -p pitgun-core --example observed_segment_aggregation
```

It groups observed engine-speed samples by `lap_id` and emits one JSON summary
per lap. The processor itself is domain-neutral: a segment key could instead be
a session, operating interval, market auction, or grid event.

This example is deliberately outside the deterministic simulation kernel. It
preserves a useful data-engineering capability for a future workflow that
aligns simulated and observed telemetry, without claiming that live input is
deterministic.

## Manifest status

The former Pipeline, Analysis, Bolt, and Bundle YAML files were research
prototypes. They did not describe the current end-to-end simulation contract,
several had no runtime consumer, and some referenced APIs or datasets that no
longer existed. They were removed rather than advertised as stable examples.

A future public run manifest should describe the complete Pitgun lifecycle:
model and scenario identity, seeded execution, emitted telemetry, analysis,
Run Bundle artifacts, replay, and verification. Observed-data ingestion and
simulation-to-operation comparison can then be optional extensions. No
compatibility with the historical prototype formats is promised.

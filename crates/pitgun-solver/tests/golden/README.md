# Racing golden runs

`racing_run_v1.input.json` is the canonical request used to protect the current
Racing JSON/WASM boundary while the simulation kernel moves between crates.
`racing_run_v1.expected.json` deliberately stores a compact observable summary
instead of the complete telemetry stream.

The target cross-runtime guarantees, run identity, and digest rules are defined
by [`DeterministicRunContractV1`](../../../../docs/DETERMINISTIC_RUN_CONTRACT_V1.md).
The fixture is the executable compatibility guard while the typed contract,
canonical digests, and stable RNG identifiers are implemented.

The fixture covers:

- race and lap timing;
- standings;
- telemetry batching, cadence, sequence, and lap metadata;
- the ordered canonical parameter identifiers exposed to the gateway.

Run it natively:

```sh
cargo test -p pitgun-solver --test racing_golden
```

Run the same test through Node/WASM:

```sh
wasm-pack test --node crates/pitgun-solver
```

Do not refresh the expected file to make a failing test pass. An intentional
observable change must first update the model or contract version and explain
the compatibility impact in the pull request.

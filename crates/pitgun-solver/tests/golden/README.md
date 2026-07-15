# Racing golden runs

`racing_run_v1.input.json` is the canonical request used to protect the current
Racing JSON/WASM boundary while the simulation kernel moves between crates.
`racing_run_v1.expected.json` retains the original compact observable summary.
The portable-exact evidence is split into additional versioned artifacts:

- `racing_run_v1.contract.json` binds the logical run identity;
- `racing_run_v1.output.json` is the complete canonical Racing domain result;
- `racing_run_v1.telemetry-summary.json` is the domain-neutral V1 summary;
- `racing_run_v1.digests.json` publishes `run_id`, `output_digest`, and
  `telemetry_summary_digest`.

The target cross-runtime guarantees, run identity, and digest rules are defined
by [`DeterministicRunContractV1`](../../../../docs/DETERMINISTIC_RUN_CONTRACT_V1.md).
The same test compares the readable canonical artifacts before their hashes in
native Rust and Node/WASM. A failure therefore identifies whether the Racing
output or telemetry summary changed before reporting the digest vector.

The fixture covers:

- race and lap timing;
- standings;
- telemetry batching, cadence, sequence, and lap metadata;
- the ordered canonical parameter identifiers exposed to the gateway;
- exact canonical output and telemetry summary digests;
- run identity derived from the typed deterministic contract;
- clear failures for contract, output, and summary mutations.

The fixture's `input.digest` is calculated from the canonical input bytes. Its
model and data-pack digest values are fixed conformance identities for this
portable-exact test. They are not runtime binary attestation; canonical artifact
manifest production remains a separate acceptance criterion in the V1 contract.
They are SHA-256 vectors over the exact UTF-8 labels
`pitgun.racing:model:1.0.0:conformance-vector` and
`pitgun.racing.2026:data-pack:1.0.0:conformance-vector`, respectively.

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

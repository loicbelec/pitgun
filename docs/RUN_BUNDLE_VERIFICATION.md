# Loaded Run Bundle Verification

`pitgun-runtime` verifies deterministic Run Bundle V1 evidence without reading
files or using a network. Application adapters load and parse artifacts, then
pass borrowed typed values and exact stored bytes to
`verify_loaded_run_bundle`.

## Runtime Responsibilities

The runtime verifies:

1. the fixed V1 manifest layout and media types;
2. every content-addressed artifact digest;
3. the contract-derived `run_id` against the manifest;
4. scenario, model, data-pack, and canonical input bindings;
5. the execution receipt against the contract and manifest evidence;
6. global telemetry ordinals and contiguous batch ordinals;
7. the telemetry summary recalculated from the ordered typed frames.

`verify_run_bundle_artifacts` is also exposed so an adapter can reject modified
raw bytes before parsing their semantic content. The complete verifier repeats
that check as a defense-in-depth invariant.

## Adapter Responsibilities

The CLI remains responsible for:

- validating the bundle root and preventing symlink or path escapes;
- reading exact artifact bytes from the filesystem;
- requiring canonical JSON and parsing JSONL records into contract types;
- extracting domain-neutral identities and the request digest from the
  scenario wrapper;
- recalculating derived metrics through `pitgun-core`;
- mapping structural replay failures and evidence mismatches to CLI exit codes.

This boundary lets a browser, service, test runner, or future worker call the
same verifier with bytes obtained through a different storage adapter. No
filesystem type or CLI presentation concept enters `pitgun-runtime`.

## Scope

The verifier proves internal consistency of the supplied evidence. Run Bundle
V1 receipts are not signatures, remote attestations, or proof that a trusted
machine performed the execution. Authority signatures and hosted trust policy
remain separate concerns.

# Deterministic Run Contract V1

Status: architecture contract for implementation

`DeterministicRunContractV1` defines the identity, execution semantics, and
verification evidence of a deterministic Pitgun run. It is domain-neutral. The
racing simulator is the first conformance workload, not part of the generic
contract.

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

## 1. Guarantees and non-goals

Given the same valid contract and canonical input, a conforming implementation
MUST produce an output that satisfies the contract's comparison profile. A
portable-exact run MUST have identical canonical output and telemetry summary
bytes on native Rust and WASM.

V1 does not provide:

- a cryptographic proof that an untrusted client executed the declared model;
- protection against a modified client, forged output, or withheld telemetry;
- fleet scheduling or server orchestration;
- meaning for domain fields such as a vehicle, circuit, or electrical asset.

A signature authorizes immutable contract bytes. It proves who authorized those
bytes, not that a submitted result was computed correctly. Correctness requires
deterministic re-execution, comparison, or an explicitly separate proof system.

## 2. Contract shape

The logical run contract has the following shape. Implementations MUST reject
unknown fields so that a typo cannot silently change run identity.

```json
{
  "contract_version": "pitgun.deterministic-run/v1",
  "scenario": {
    "id": "racing.single-lap",
    "version": "1.0.0"
  },
  "model": {
    "id": "pitgun.racing",
    "version": "1.0.0",
    "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "data_pack": {
    "id": "pitgun.racing.2026",
    "version": "1.0.0",
    "digest": "sha256:123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0"
  },
  "runtime_profile": "portable-exact-v1",
  "random": {
    "seed": "7",
    "algorithm": "pitgun-splitmix64-v1",
    "stream_derivation": "sha256-label-v1"
  },
  "clock": {
    "kind": "logical-fixed-step",
    "epoch": 0,
    "tick_numerator_us": 50000,
    "tick_denominator": 1
  },
  "event_ordering": {
    "keys": ["logical_tick", "source_id", "source_sequence", "insertion_ordinal"],
    "string_order": "unicode-code-point"
  },
  "input": {
    "media_type": "application/json",
    "canonicalization": "jcs-rfc8785",
    "digest": "sha256:23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01"
  }
}
```

The hexadecimal values above are illustrative. A producer MUST compute them
from the actual artifacts and canonical bytes; it MUST NOT copy example values.

### 2.1 Field rules

| Field | Canonicalization and validation rule |
|---|---|
| `contract_version` | Exact ASCII constant `pitgun.deterministic-run/v1`. |
| `*.id` | Lowercase ASCII matching `[a-z0-9][a-z0-9._-]{0,127}`. IDs are immutable names, not URLs. |
| `*.version` | Exact SemVer without a range or leading `v`. A different version denotes different semantics. |
| `model.digest` | Lowercase `sha256:` plus 64 hexadecimal characters, computed over a canonical model manifest shared by all supported targets. Compiled binaries are identified separately by `runtime.artifact_digest`. |
| `data_pack.digest` | Same encoding, computed over a canonical manifest containing every path and content digest in Unicode code-point path order. |
| `runtime_profile` | One of the comparison profiles in section 7. The profile is versioned and immutable. |
| `random.seed` | Canonical decimal string representing a `u64`, matching `0|[1-9][0-9]{0,19}` and bounded by `18446744073709551615`. Leading zeroes and signs are forbidden. Binary formats SHOULD encode the same value as `u64`. |
| `random.algorithm` | Exact versioned algorithm identifier. Aliases such as `default` or library types such as `StdRng` are forbidden. |
| `random.stream_derivation` | Exact versioned rule used to derive independent streams from the root seed. |
| `clock.kind` | Exact value `logical-fixed-step` in V1. Wall-clock time MUST NOT drive model evolution. |
| `clock.epoch` | Signed integer logical origin. V1 racing uses zero microseconds. |
| `tick_numerator_us` | Positive integer numerator of the logical tick duration in microseconds. |
| `tick_denominator` | Positive integer denominator. The fraction MUST be reduced to lowest terms. |
| `event_ordering.keys` | Ordered, non-empty list defining a total order. V1 uses the four keys shown above in that order. |
| `event_ordering.string_order` | Exact value `unicode-code-point`; strings are compared by Unicode scalar value after input validation. |
| `input.media_type` | Lowercase registered media type without optional parameters. V1 supports `application/json`. |
| `input.canonicalization` | Exact canonicalization algorithm identifier. V1 supports `jcs-rfc8785`. |
| `input.digest` | SHA-256 of the canonical input bytes, using the same lowercase encoding as artifact digests. |

All identifiers and versions are part of run identity. Mutable labels, build
dates, host names, request times, user names, and authorization expiry are not.
They belong in execution or authorization metadata.

## 3. Canonical JSON and digests

JSON input, contract, output, and telemetry summaries use the JSON
Canonicalization Scheme defined by [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785).

Before canonicalization, a V1 implementation MUST:

1. decode UTF-8 strictly and reject duplicate object keys;
2. reject invalid Unicode and require strings to already be NFC-normalized;
3. reject `NaN`, positive or negative infinity, and values that cannot be
   represented by the declared schema;
4. validate the domain schema and reject unknown fields;
5. preserve array order unless the schema explicitly defines the array as a set;
6. canonicalize schema-defined sets by their documented key before hashing.

Objects are serialized in RFC 8785 key order with no insignificant whitespace.
JSON numbers use its ECMAScript-compatible shortest representation. Domain
schemas SHOULD use integers, fixed-point integers, or bounded decimal fields for
values that affect portable-exact results. A producer MUST NOT hash the source
file bytes, pretty-printed JSON, or a language-specific map iteration order.

The digest operation is:

```text
digest(value) = "sha256:" + lowercase_hex(SHA-256(JCS(value)))
```

Digests compare canonical content. They do not replace schema, model, or data
pack version identifiers: both a semantic version and a digest are required.

## 4. Run identity and execution identity

`run_id` identifies a logical computation independently of where it ran:

```text
run_id = digest(<complete DeterministicRunContractV1>)
```

A verifier can construct the identity without loading the input artifact, but
MUST validate the artifact against `input.digest` before execution.

The native and WASM executions of the same portable contract therefore share a
`run_id`. Each concrete attempt has a separate receipt:

```json
{
  "run_id": "sha256:3456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef012",
  "execution_id": "018f3b78-7e9a-7d20-a5e1-4ed92f02a591",
  "runtime": {
    "engine": "pitgun-rust",
    "engine_version": "1.82.0",
    "target": "wasm32-unknown-unknown",
    "artifact_digest": "sha256:456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123"
  },
  "output_digest": "sha256:56789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01234",
  "telemetry_summary_digest": "sha256:6789abcdef0123456789abcdef0123456789abcdef0123456789abcdef012345"
}
```

`execution_id` is an opaque UUIDv7 generated for observability and replay audit.
It MUST NOT influence simulation. `runtime.engine`, `engine_version`, `target`,
and `artifact_digest` identify the code that actually ran and MUST be recorded
with exact values. They are evidence, not inputs to logical run identity.

Receipt fields use these canonical rules:

| Field | Canonicalization and validation rule |
|---|---|
| `run_id` | Lowercase SHA-256 encoding calculated from the complete contract; a submitted value MUST be recomputed. |
| `execution_id` | Lowercase canonical UUIDv7 string. It MUST be unique per attempt. |
| `runtime.engine` | Stable lowercase ASCII ID using the same grammar as contract IDs. |
| `runtime.engine_version` | Exact SemVer for the executing implementation. |
| `runtime.target` | Exact lowercase target triple from the runtime registry; aliases are forbidden. |
| `runtime.artifact_digest` | Lowercase SHA-256 encoding of the exact native binary or WASM module that executed. |
| `output_digest` | Lowercase SHA-256 encoding of the complete canonical domain output. |
| `telemetry_summary_digest` | Lowercase SHA-256 encoding of the canonical summary defined in section 8. |

## 5. Randomness

All randomness MUST descend from `random.seed`. Models MUST NOT read ambient
entropy, process IDs, thread scheduling, wall-clock time, or platform RNGs.

The algorithm identifier specifies the complete bit-generation algorithm and
its version. A standard library wrapper whose algorithm may change, including
Rust `StdRng`, is not a valid durable identifier. The stream derivation rule
MUST combine the root seed with stable UTF-8 labels and stable integer indexes;
it MUST NOT depend on call order across independent components.

The target V1 algorithm identifiers are:

- `pitgun-splitmix64-v1` for the explicitly specified 64-bit generator;
- `sha256-label-v1` for deriving a stream seed from
  `JCS([root_seed, component_id, entity_id, logical_index])`.

The implementation ticket for these identifiers MUST publish test vectors.
Until that work lands, the current Racing golden run detects regressions but is
not yet a durable RNG compatibility guarantee.

## 6. Clock and event ordering

Simulation time is rational logical time. Tick `n` occurs at:

```text
epoch + n * tick_numerator_us / tick_denominator
```

Implementations MUST calculate scheduling and ordering with integer or rational
arithmetic. Conversion to a telemetry timestamp MUST use a schema-defined
rounding rule; V1 uses nearest integer microsecond with ties away from zero.

Events at the same logical tick are ordered by this total key:

1. `logical_tick`, ascending integer;
2. `source_id`, ascending Unicode code point;
3. `source_sequence`, ascending unsigned integer;
4. `insertion_ordinal`, ascending unsigned integer assigned by the producer
   before concurrency or transport.

Missing keys, duplicate complete keys, and integer overflow are errors. A model
MUST NOT use hash-map iteration order, thread completion order, arrival time, or
database row order as a tie-breaker.

## 7. Native and WASM comparison profiles

### 7.1 `portable-exact-v1`

This is the default and required profile for the Racing reference workload.

- Canonical outputs and telemetry summaries MUST be byte-for-byte identical on
  supported native and WASM targets.
- `output_digest` and `telemetry_summary_digest` MUST match exactly.
- Contract-visible time, counters, positions, identifiers, and final metrics
  MUST be integers, fixed-point values, booleans, or strings.
- Floating-point intermediates are allowed only when the model defines explicit
  finite input bounds, operation order, and rounding checkpoints before values
  become contract-visible.
- Fused operations, fast-math, approximate intrinsics, and platform-dependent
  reductions MUST NOT change observable results.

Conformance requires the same fixture to run in native Rust and Node/WASM CI.
The current `racing_run_v1` golden summary is the first such fixture.

### 7.2 `bounded-float-v1`

This profile is available for models where exact cross-runtime floating-point
output is not practical. Its contract MUST additionally carry a versioned
comparison manifest. Each non-exact field is selected by JSON Pointer and has:

- an absolute tolerance;
- a relative tolerance;
- an optional quantization step;
- an explicit rule for signed zero.

Comparison passes when, for finite expected value `e` and actual value `a`:

```text
abs(a - e) <= max(absolute_tolerance, relative_tolerance * abs(e))
```

`NaN` and infinity always fail. Arrays and events still require exact length and
order. Fields absent from the comparison manifest use exact equality.

In this profile, artifact digests identify exact bytes from one execution but
MUST NOT be used alone to claim cross-runtime equivalence. Verification means
the structured tolerance comparison passed under the exact manifest version.

Changing a tolerance, JSON Pointer, quantization step, or equality mode creates
a new comparison-manifest version and a different `run_id`.

## 8. Output and telemetry evidence

`output_digest` is the digest of the complete canonical domain output.
`telemetry_summary_digest` is the digest of a canonical domain-neutral summary:

```json
{
  "schema_version": "pitgun.telemetry-summary/v1",
  "batch_count": 7,
  "frame_count": 427,
  "first_timestamp_us": 0,
  "last_timestamp_us": 85200000,
  "first_sequence": 0,
  "last_sequence": 426,
  "parameter_ids": [5000, 5001, 5002],
  "event_count": 0,
  "dropped_frame_count": 0
}
```

The summary schema is versioned. Parameter IDs are sorted numerically and
deduplicated; counts and sequence values are unsigned integers; absent streams
use `null` for first and last values. A domain MAY extend the summary in a
namespaced `domain` object whose schema and version are included in the object.

For audits that require the full telemetry stream, a receipt MAY also contain
`telemetry_digest`. Frames are then canonicalized individually in total event
order, length-prefixed as unsigned 64-bit big-endian byte counts, concatenated,
and hashed. This avoids ambiguity between adjacent serialized frames.

## 9. Complete Racing example

The Racing reference binds the existing
`crates/pitgun-solver/tests/golden/racing_run_v1.input.json` fixture as follows:

```json
{
  "contract_version": "pitgun.deterministic-run/v1",
  "scenario": {"id": "racing.single-lap", "version": "1.0.0"},
  "model": {
    "id": "pitgun.racing",
    "version": "1.0.0",
    "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "data_pack": {
    "id": "pitgun.racing.2026",
    "version": "1.0.0",
    "digest": "sha256:123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0"
  },
  "runtime_profile": "portable-exact-v1",
  "random": {
    "seed": "7",
    "algorithm": "pitgun-splitmix64-v1",
    "stream_derivation": "sha256-label-v1"
  },
  "clock": {
    "kind": "logical-fixed-step",
    "epoch": 0,
    "tick_numerator_us": 50000,
    "tick_denominator": 1
  },
  "event_ordering": {
    "keys": ["logical_tick", "source_id", "source_sequence", "insertion_ordinal"],
    "string_order": "unicode-code-point"
  },
  "input": {
    "media_type": "application/json",
    "canonicalization": "jcs-rfc8785",
    "digest": "sha256:23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01"
  }
}
```

The expected observable result is:

```json
{
  "total_time_ms": 85284,
  "player_lap_times_ms": [85284],
  "standings": [
    {
      "competitor_id": "player",
      "position": 1,
      "total_time_ms": 85284,
      "best_lap_ms": 85284,
      "laps_completed": 1,
      "gap_to_leader_ms": 0,
      "status": "finished"
    }
  ],
  "telemetry": {
    "batch_count": 7,
    "frame_count": 427,
    "first_timestamp_us": 0,
    "last_timestamp_us": 85200000,
    "first_sequence": 0,
    "last_sequence": 426,
    "samples_per_frame": 17,
    "parameter_ids": [
      5000, 5001, 5002, 5003, 5004, 5005, 5006, 5007, 5008,
      5009, 5010, 5011, 5012, 5013, 5014, 5015, 5016
    ],
    "first_lap_number": 1,
    "last_lap_number": 1,
    "source_id": "pitwall-sim:player",
    "sampling_hz": "5"
  }
}
```

The example is complete at the semantic level but its artifact and content
digests remain illustrative until the canonicalization and stable RNG
implementation tickets land. The checked-in golden fixture remains the current
executable source of truth during that transition.

## 10. Replay and compatibility

A replay request MUST contain the complete contract, canonical input artifact,
and expected receipt. Before execution, a verifier MUST validate all referenced
digests, supported versions, authorization constraints, and schema rules.

Version identifiers are immutable:

- a compatible implementation fix that leaves every observable byte unchanged
  MAY keep the same semantic component version but produces a new runtime
  artifact digest;
- a change to canonical input, model semantics, data, RNG, clock, event order,
  comparison rules, output schema, or telemetry summary MUST change the
  corresponding version or digest and therefore the `run_id`;
- old fixtures MUST NOT be overwritten in place;
- unsupported versions MUST fail closed with a machine-readable error, never
  silently fall back to the newest implementation.

Deprecation is registry metadata, not a reinterpretation of a version. A
registry entry records `supported`, `deprecated`, or `revoked` plus a reason and
date. Deprecated runs remain replayable when artifacts are retained. Revoked
runs remain identifiable but MUST NOT be newly authorized.

## 11. Untrusted client threat model

An untrusted client can modify code, fabricate a receipt, replay an old valid
submission, omit telemetry, or search seeds and inputs offline. V1 therefore
requires the server boundary to:

- verify the authority signature over authorization bytes;
- validate expiry, subject, audience, nonce, allowed model/data versions,
  input digest, seed, and resource limits;
- bind the submission to the signed authorization and reject nonce reuse;
- recompute the canonical contract, `run_id`, and submitted digests;
- deterministically re-execute all high-value runs, or sample lower-value runs
  according to an explicit policy;
- rate-limit and bound input, output, telemetry, duration, and replay attempts;
- retain the runtime receipt and verification decision for audit.

Authorization fields such as subject, audience, nonce, issuance, and expiry MAY
wrap a deterministic contract, but MUST NOT alter the logical simulation unless
they are explicitly copied into canonical input. A valid signature without
re-execution is authorization, not proof of correct execution.

## 12. Stable implementation acceptance criteria

Follow-up work can cite these requirements directly:

1. **Schema and canonicalization:** reject duplicate/unknown fields and invalid
   numbers; implement RFC 8785 canonical bytes and SHA-256 test vectors.
2. **Identity:** implement typed contract and receipt models; prove reordered
   JSON gives the same `run_id` and a semantic field change gives a different
   one.
3. **Artifacts:** produce canonical model and data-pack manifests and verify
   their digests before execution.
4. **RNG:** replace ambient or library-default RNG identity with the named V1
   algorithm and publish native/WASM test vectors.
5. **Clock/order:** remove wall-clock and collection-order dependencies; test
   simultaneous events and tie-breakers.
6. **Portable exact:** run the same Racing fixture natively and in Node/WASM and
   compare canonical output and telemetry summary digests exactly.
7. **Bounded float:** implement manifest validation and boundary tests for
   absolute/relative tolerance, signed zero, non-finite values, arrays, and
   missing fields.
8. **Security boundary:** sign authorization separately from execution receipt;
   test expiry, nonce replay, digest mismatch, unsupported version, and forged
   receipt rejection.
9. **Replay:** retain versioned fixtures and fail closed when an artifact or
   semantic component is unavailable.

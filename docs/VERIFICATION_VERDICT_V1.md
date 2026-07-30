# Verification Verdict V1

`VerificationVerdictV1` is the durable, server-owned decision for one concrete
execution attempt. It lets persistence and presentation layers expose hosted
verification without implementing or trusting verification themselves.

The verdict is domain-neutral. Racing is the first workload producing it, but
no circuit, vehicle, game, career, or leaderboard field belongs to this
contract.

## Identity boundaries

- `run_id` identifies the deterministic computation.
- `execution_id` identifies the concrete native, WASM, or worker attempt.
- `submitted_evidence` identifies the exact receipt, output, and telemetry
  summary bytes received for that attempt.
- `verifier` identifies the exact trusted implementation that produced the
  record.
- `recorded_at_ms` is operational audit metadata and never changes `run_id`.

The verifier calculates submitted evidence digests itself. A client cannot
submit a trusted status or promote an execution to `VERIFIED`.

## States

### `PENDING`

Verification has not reached a terminal decision. A retryable `reason_code` is
required and `verified_resolution` is absent.

Examples include queued work, a temporarily unavailable catalog archive, an
unavailable model store, or another verifier dependency outage. Retrying the
same execution must not create a different semantic result.

### `VERIFIED`

Authorization, evidence integrity, resource resolution, and required
deterministic replay checks all succeeded. No failure reason is present.

`verified_resolution` records the exact model, data pack, and authority policy
accepted by the verifier. These identities are safe lineage metadata because
the verifier resolved and validated them independently.

### `REJECTED`

A terminal fail-closed check failed. A rejection `reason_code` is required and
`verified_resolution` is absent. Unknown model, pack, or policy identities are
rejections; transient inability to retrieve a known resource remains pending.

## Consumer projections

The Racing leaderboard may expose:

- status;
- `run_id` and `execution_id`;
- verifier identifier and version;
- verification timestamp;
- verified model, data-pack, and policy identifiers and versions;
- submitted evidence digests;
- a player-safe reason code.

The Pit Wall may additionally present the operational progression from
authorization through simulation, upload, pending verification, and the final
verdict.

Detached authorization signatures, secret material, internal dependency
locations, and diagnostic error strings are not presentation data. A technical
view may offer this versioned verdict as JSON, but it must not imply that the
verdict replaces the underlying signed authorization and retained evidence.

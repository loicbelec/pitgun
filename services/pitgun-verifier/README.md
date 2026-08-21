# pitgun-verifier

`pitgun-verifier` independently validates deterministic execution evidence. The
Racing workload is its first hosted adapter.

The verification engine:

1. verifies the Authority signature, audience, validity window, and `run_id`;
2. checks the strict Racing V1 contract profile;
3. resolves the statically linked model and retained immutable Simulation Pack;
4. checks the retained Authority policy identity;
5. recalculates receipt, output, and telemetry-summary digests;
6. binds the concrete `execution_id` to the authorized run;
7. replays the exact Racing workload through `pitgun-runtime`;
8. emits `VerificationVerdictV1`.

For the non-production Model V3 thermal candidate, submitted lineage also
contains the exact catalog release, Simulation Pack, model, and reviewed
thermal-family profile digest. The Verifier reconstructs that projection from
its retained catalog before replay and rejects substitutions.

## Catalog-backed and direct-pack runs

The signed deterministic contract contains the immutable Simulation Pack
identity, not the mutable catalog discovery path. A catalog-backed Authority
request and a direct immutable-pack request therefore have the same verifier
semantics when they select the same pack.

The verifier resolves that exact identity from its retained catalog/archive.
Missing retained bytes produce a retryable `PENDING` decision. A present but
different identity produces terminal `REJECTED`.

## Boundaries

The pure engine does not:

- trust a client-provided verdict;
- persist leaderboard rows;
- consume an authorization nonce;
- claim durable idempotency;
- perform mutable `latest` discovery;

The `pitgun-verifier` binary exposes an internal worker API:

- `GET /healthz`;
- `GET /readyz`;
- `POST /v1/verifications/racing`.

`PENDING` uses HTTP 202. Terminal `VERIFIED` and `REJECTED` decisions use HTTP
200 because both are successfully processed, server-owned verdicts. Malformed
transport input and internal failures are not verdicts.

Deterministic replay runs on bounded blocking workers. Configure the limit with
`PITGUN_VERIFIER_MAX_CONCURRENT_REPLAYS`; the VPS-oriented default is `2`.

The worker loads:

- retained Authority HMAC verification material from
  `PITGUN_SIGNING_SECRET_FILE` (or inline material only for local development);
- its retained key identifier from `PITGUN_SIGNING_KEY_ID`;
- its expected audience from `PITGUN_VERIFIER_AUDIENCE`;
- its exact Racing generation from `PITGUN_RACING_MODEL_VERSION` (V1 by default,
  with `0.10.0` reserved for the non-production V3 thermal candidate);
- the immutable Racing release from `PITGUN_RACING_CATALOG_RELEASE_DIR`;
- the accepted tuning policy from `PITGUN_TUNING_POLICY_PATH`.

`/readyz` fails closed when signing-key verification material or retained
catalog bytes are unavailable.

The published container retains the immutable Racing `v1.0.0`, `v1.2.0`,
`v1.3.0`, and non-production `v1.5.0` releases. Environments select one exact
release directory together with its compatible model generation; the image
never follows mutable catalog discovery.

The HTTP endpoint is an internal boundary. It must not be routed directly to a
browser. Durable nonce consumption and idempotent verdict persistence belong to
the game/backend submission transaction. Keeping the engine separate also
allows the worker to move from the VPS to another machine, including a future
Mac mini, without changing the verdict contract.

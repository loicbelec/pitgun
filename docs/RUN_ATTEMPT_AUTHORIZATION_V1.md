# Dynamic Run Attempt Authorization V1

`RunAttemptAuthorizationV1` is the domain-neutral security envelope for a
deterministic execution whose final canonical input is not completely known at
issuance.

It complements rather than replaces `RunAuthorizationV1`:

- `RunAuthorizationV1` authorizes one immutable computation and signs its final
  `run_id` before execution;
- `RunAttemptAuthorizationV1` authorizes one `execution_id`, an immutable
  initial contract, and a bounded domain decision envelope before the final
  `run_id` exists.

## Signed boundary

The canonical authorization binds:

- a nonce, subject, audience and exact `execution_id`;
- the complete initial `DeterministicRunContractV1` and redundant
  `initial_run_id`;
- one content-addressed `decision_envelope` interpreted by the domain workload;
- policy and signing-key identities;
- execution expiry and late-submission grace.

The generic contract never parses Racing modes, competitors or lap boundaries.
For Racing, the opaque identity addresses the canonical
`RacingDriverInstructionAuthorizationV1` bytes. Another workload may bind a
different strict envelope without changing this authorization schema.

## Completed identity

During execution, accepted domain decisions enter a canonical completed input.
The final contract may differ from the initial contract only through
`input.digest`. Its input media type and canonicalization, scenario, model,
data pack, runtime profile, seed, clock and event ordering remain exact.

The workload verifier must:

1. verify the signed attempt authorization and time window;
2. resolve and digest the exact domain decision envelope;
3. validate the completed domain input against that envelope;
4. derive the final deterministic contract and `run_id`;
5. check that the receipt uses both that final `run_id` and the authorized
   `execution_id`;
6. atomically consume the nonce when accepting the submission;
7. replay the canonical completed input independently.

`execution_id` therefore remains stable from authorization through completion,
while `run_id` describes the exact result after all accepted deterministic
decisions are known.

## Compatibility and availability

This is a new wire type. It does not reinterpret or change the signing bytes of
`RunAuthorizationV1`, existing receipts, or published Hosted Verification
evidence.

Authority is required to issue a new attempt. Once signed, an already-started
attempt may finish and be submitted within its grace window even if Authority
is unavailable. Locally executed work outside a signed envelope remains
unverified and cannot enter the verified leaderboard retroactively.

`pitgun-signing::VerificationKeyring` verifies immutable and dynamic
authorizations with the same active and retained HMAC keys. Dynamic execution
and submission checks reuse the existing audience, expiry and grace semantics.
No additional secret, environment variable, key store or rotation procedure is
introduced. Cryptographic verification deliberately does not consume the
nonce; the accepting persistence transaction remains the replay-protection
boundary.

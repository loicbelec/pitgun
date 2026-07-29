# Deterministic Run Authorization V1

`RunAuthorizationV1` is the authority-owned security envelope around one
`DeterministicRunContractV1`. It authorizes immutable computation bytes without
polluting logical simulation identity with operational metadata.

## Identity boundary

The deterministic identity remains:

```text
run_id = SHA-256(JCS(DeterministicRunContractV1))
```

The authorization additionally signs:

- a random single-use nonce;
- subject and audience;
- the complete deterministic contract and redundant `run_id`;
- the exact policy artifact;
- the signing-key identifier;
- issuance, execution expiry and late-submission grace.

Changing authorization metadata does not change `run_id`. Changing the
scenario input, seed, model, Simulation Pack, runtime profile, clock or event
ordering does.

The verifier must recompute `run_id` before signature verification. The
authority signs RFC 8785 canonical bytes returned by
`RunAuthorizationV1::signing_bytes`.

## Verification sequence

A trusted verifier fails closed unless it can:

1. parse the strict versioned authorization without unknown fields;
2. recompute the declared `run_id`;
3. match the expected audience;
4. select retained verification material by `signing_key_id`;
5. verify the signature over canonical authorization bytes;
6. validate execution expiry or the distinct submission-grace deadline;
7. validate the canonical input against `contract.input.digest`;
8. atomically consume the signed nonce when accepting a result;
9. replay or sample the deterministic execution according to policy.

A signature proves authorization, not correct execution.

## Availability semantics

The authority is required only to issue new authorizations. Once issued, the
contract and its signature are self-contained and remain verifiable while the
authority is unavailable.

During an outage:

- already-authorized work may finish;
- its result may be accepted within the signed late-submission grace;
- local unverified work may continue as a user experience;
- new official work cannot be authorized;
- a locally invented run is never signed retroactively.

This prevents an authority outage from destroying ongoing play while keeping
the verified leaderboard fail-closed.

## HMAC V1 and rotation

V1 declares `hmac-sha256`. Authority and verifier therefore share a trusted
secret boundary. Private material must never reach a browser or log.

The authority uses one active key ID. The verifier uses
`VerificationKeyring`, adds the next key before rotation, and retains previous
keys until all authorizations they signed have passed expiry plus grace.

HMAC is not suitable for independently operated public verifiers because every
verifier holding the secret can forge authority output. Such a deployment must
introduce a new versioned asymmetric signature algorithm rather than changing
V1 semantics in place.

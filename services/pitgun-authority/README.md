# pitgun-authority

`pitgun-authority` canonicalizes domain input and authorizes immutable
deterministic runs. A signature proves which bytes the authority accepted. It
does not prove that an untrusted client computed the submitted result
correctly; the verifier still replays or samples executions.

## Racing deterministic authorization

`POST /v1/authorizations/racing` accepts a Racing run input, a stable subject
and either:

- an exact immutable `catalog_release` already loaded by the authority; or
- an explicit `data_pack` for a catalog-free workload.

Mutable discovery URLs, `latest`, and Presentation Pack identity are never
accepted as deterministic identity.

```json
{
  "subject": "398105f6-2f4b-4874-9a1f-0f3aa1ee9d05",
  "seed": "42",
  "catalog_release": {
    "schema_version": "pitgun.catalog-release-identity/v1",
    "id": "pitgun.racing",
    "version": "1.0.0",
    "manifest_digest": "sha256:5f36612a3ce265750051ca913747c0c05bc08a534dc307bbc7b02b8cb2f6e156"
  },
  "input": {
    "track_id": "LASVEGAS",
    "laps": 50,
    "competitors": [],
    "vehicle_id": "f1_2026",
    "era": 6,
    "hz": 20
  }
}
```

The response contains:

- `canonical_input`, after Racing policy normalization;
- a strict `DeterministicRunContractV1`;
- its recomputable `run_id`;
- the exact model, Simulation Pack and policy identities;
- subject, audience, nonce, validity and signing-key ID;
- an HMAC-SHA256 signature over RFC 8785 canonical authorization bytes.

The catalog release is returned as resolution metadata, outside `run_id`. The
contract binds its Simulation Pack through `data_pack`.

## Availability and delayed submission

An authority outage has deliberately asymmetric behavior:

1. a previously issued authorization remains independently verifiable;
2. a player may finish an already-authorized execution;
3. its result may be submitted until `expires_at_ms +
   late_submission_grace_ms`;
4. a new local run may remain playable, but cannot enter the verified
   leaderboard;
5. the authority never retroactively signs a run invented while it was
   unavailable.

The client may keep an unverified result locally and retry a normal submission,
but it must not label that result as verified.

## Replay protection

Every issuance receives a cryptographically random signed `nonce`. Signature
verification does not consume it. The verifier persistence boundary must
atomically record the nonce when accepting a result and reject any later reuse.

## Key rotation

The active authority key is selected by:

- `PITGUN_SIGNING_SECRET`: private HMAC material;
- `PITGUN_SIGNING_KEY_ID`: stable public identifier included in signed bytes.

To rotate:

1. create a new random secret and a new key ID;
2. deploy them to the authority;
3. add the new pair to verifier configuration before issuing with it;
4. retain the previous verifier key until the last authorization it signed has
   passed expiry plus late-submission grace;
5. remove the old key only after that deadline.

The browser receives the key ID and signature, never HMAC material. `SigningKey`
also redacts its `Debug` representation. HMAC is appropriate while authority
and verifier share one trusted deployment boundary. A future independently
operated or publicly verifiable ecosystem should version a new asymmetric
signature algorithm rather than distributing the HMAC secret.

## Configuration

| Variable | Default | Meaning |
|---|---:|---|
| `PITGUN_SIGNING_SECRET` | required | Active private HMAC material |
| `PITGUN_SIGNING_KEY_ID` | `pitgun-authority-v1` | Active verification-key ID |
| `PITGUN_AUTHORITY_AUDIENCE` | `pitgun.verifier` | Only intended verifier |
| `PITGUN_SIM_CONTRACT_TTL_SECONDS` | `300` | New-execution authorization window |
| `PITGUN_LATE_SUBMISSION_GRACE_SECONDS` | `900` | Additional result-submission window |
| `PITGUN_ALLOW_CATALOG_FREE` | `false` | Permit explicitly identified non-catalog data packs |
| `PITGUN_TUNING_POLICY_PATH` | `policies/gametuning.v1.yaml` | Exact Racing policy bytes |
| `PITGUN_RACING_CATALOG_RELEASE_DIR` | unset | Optional immutable Racing release directory |
| `PITGUN_AUTHORITY_BIND` | `0.0.0.0:8080` | HTTP listener |

`/readyz` returns `503` when signing material is unavailable. `/healthz`
reports only process liveness.

## Legacy setup contract

`POST /v1/contracts/simulation` remains temporarily available for existing
consumers of `SimulationContractV1`. New integrations must use the deterministic
authorization endpoint.

## Container image

The dedicated image is published as:

```text
ghcr.io/loicbelec/pitgun-authority:<git-commit-sha>
```

It contains the authority binary plus the exact checked-in policy and immutable
Racing `v1.0.0` catalog release:

```text
/opt/pitgun/policies/gametuning.v1.yaml
/opt/pitgun/catalogs/racing/v1.0.0
```

The corresponding path variables are configured as non-secret image defaults.
`PITGUN_SIGNING_SECRET`, key rotation and environment-specific routing remain
deployment concerns and are never baked into the image.

Pull requests build and smoke-test the container without publishing it. A merge
to `main` publishes only the immutable commit-SHA tag consumed by
`infra-vps`.

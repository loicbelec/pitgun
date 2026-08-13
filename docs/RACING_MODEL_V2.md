# Racing model V2

Racing model V2 promotes the reviewed continuous curvature response as an
explicit deterministic workload. It does not replace model V1 in place.

## Governed identities

| Model | Aerodynamic response | Compatible catalog |
|---|---|---|
| `pitgun.racing@1.0.0` | legacy binary straight/corner selection | `v1.0.0`, `v1.1.0` |
| `pitgun.racing@2.0.0` | continuous cubic curvature response | `v1.2.0` |

The complete model identity also includes its SHA-256 digest. Hosted execution
selects a workload only when the ID, version, digest, contract version, and
Simulation Pack identity all match. Cross-generation combinations fail closed.

V2 uses the shared curvature function in every Solver force calculation and
diagnostic. Model selection is internal to the versioned workload and cannot be
overridden by player input.

## Evidence

The native and Node/WASM golden suites protect separate V1 and V2 contracts,
outputs, telemetry summaries, receipts, and digests. The V1 fixtures remain
byte-identical. The seven-circuit, 154-run curvature-band campaign remains the
physical promotion evidence for V2.

## Rollback

Rollback is an identity selection, not a mutation of V2:

1. authorize `pitgun.racing@1.0.0` with its exact published digest;
2. select a catalog release that declares compatibility with model V1;
3. deploy matching game, Authority, and Verifier artifacts together;
4. retain V2 artifacts and evidence for replay and audit.

Moving only a catalog pointer or changing only the model version is rejected by
compatibility validation. Environment rollout and rollback rehearsal are owned
by the separate staging task #202.

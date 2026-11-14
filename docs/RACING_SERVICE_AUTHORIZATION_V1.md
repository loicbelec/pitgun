# Signed Racing service control v1

14 November 2026 · framework#415 / draft#429 · game#321 / draft#320.
Local integration; activation/publication remains part of the single staging
candidate. No physics coefficients or legacy signed contracts are changed.

## Authorization and completed identity

`POST /v1/authorizations/racing/services` uses the existing subject, execution ID,
string seed, input and exact catalog-release identity request. Authority owns the
model/catalog/runtime/policy choice, canonicalizes tuning with its current policy,
checks one player, bounded inputs and initializes the resolved setup without
advancing a lap. The caller cannot supply a permission envelope or runtime path.

`pitgun.racing-service-authorization/v1` binds the initial input, exact content-
addressed control policy, operator-allowlisted WASM digest, player, sorted catalog
tyres, race distance and decision bound. The policy fixes V9 thermal physics,
service window margin, rival thresholds, planned reviews and strategy observation.
Its artifact identity is signed into the existing generic attempt, with scenario
`racing.service-control@1.0.0`. Driver-mode attempts cannot grant this permission.

The ordered applied history consists of **native observation digest + native
applied receipt** pairs. These are float-free wire facts. Every mandatory window
must appear exactly once, including Keep/expiry; receipts cannot be inserted,
reordered or retargeted. Full observations and provider exchanges remain in the
local audit; replay reconstructs the original facts from the pinned inputs.
Consumers copy runtime-generated observation digests, never rehash floating-point
snapshots in JavaScript or a different numerical runtime.

`pitgun.racing-completed-service-input/v1` binds the authorized input, permission
envelope identity and canonical applied-history digest. Only this completed input
digest changes in the final run contract. Receipt, result and telemetry summary
then bind to that final run and the original execution ID. Signature verification
and independent replay remain mandatory; the client receipt is not authority.

## Exact replay runtime

The browser/native diagnostic found a real one-ULP engine-temperature difference
(`85.94555974244156` vs `85.94555974244157`). Native-only tests did not detect it.
No observation tolerance or physical rounding is introduced to accept it.

The hosted service route requires the **same exact WASM bytes** on the verifier.
A fixed trusted Node adapter loads only the operator's hash-pinned Pitgun WASM and
standalone bridge. It reconstructs each observation digest and domain verdict,
checks receipts and history exhaustion, then returns compact evidence projected
inside WASM. The native reference replay remains explicitly named for unit tests;
the HTTP route never falls back to it.

The control completion adds `native_evidence` (canonical output/telemetry summary)
without changing the underlying result or its raw canonical digest. The bridge
ABI is unchanged. Old artifacts retain their old bytes. Older artifacts lacking
compact evidence cannot serve as this new hosted replay runtime.

The child has a cleared environment, read permission only for its three operator-
owned files, no filesystem write/child-process permission, bounded stdin/stdout,
120-second timeout and process/thread cleanup. No URL, file path, source code,
model tool or WASM artifact is selected from an HTTP request. The verifier identity
includes the native verifier, Node binary, adapter script, WASM and bridge digests.
This is replay provenance, not hardware or LLM-provider attestation.

Resource failures/timeouts produce retryable `PENDING/dependency_unavailable`;
invalid signatures/permissions/history/results produce terminal rejections. The
existing shared replay semaphore and receipt-time admission deadline are reused.
The configured JavaScript heap limit does not itself bound WASM linear memory:
full-GP RSS under the deployment cgroup still requires qualification.

## Configuration and fail-closed activation

New routes are separate; ordinary fixed/driver-mode routes and defaults remain.
Both services need explicit compatible catalog and expected runtime digest:

- `PITGUN_RACING_SERVICE_CATALOG_RELEASE_DIR`
- `PITGUN_RACING_SERVICE_WASM_DIGEST`

Verifier additionally requires absolute operator-owned files/identities:

- `PITGUN_SERVICE_NODE_BINARY`
- `PITGUN_SERVICE_REPLAY_SCRIPT`
- `PITGUN_SERVICE_WASM_FILE`
- `PITGUN_SERVICE_BRIDGE_FILE` (standalone `.mjs`, identical pinned bridge bytes)
- `PITGUN_SERVICE_BRIDGE_DIGEST`

`GET /v1/authorizations/racing/services/readyz` and
`GET /v1/verifications/racing/services/readyz` expose capability readiness. A
legacy green `/readyz` does not imply that service control is enabled. Missing or
incompatible resources fail closed; there is no implicit runtime/model upgrade.
Signing bodies are capped at1 MiB, verifier bodies at2 MiB, replay input at32 MiB
(trusted resolved catalog included), compact output at64 KiB; first policy uses
5 Hz,2..128 laps,1..16 competitors and at most35 decisions.

The physical workload is the same component/V9 model0.11.0 as the local career.
Tests explicitly use compatible catalog1.7.0. Existing catalog1.8.0 declares only
model0.14.0 and is rejected for this route. The career staging candidate needs an
explicit compatible immutable catalog release, with validated resource parity;
do not pretend the old driver signature authorizes service control or switch the
physics to fit a signature. New schemas remain experimental and unpublished.

## Remaining release gates

The backend core is distinct from the career adapter. Persistent SIGNED admission,
career outbox/nonce consumption, publication/leaderboard projection and the hosted
Engineer/shared-quota route still need their normal adapters and end-to-end
qualification. This stateless verifier route alone neither publishes scores nor
attests offline development ownership, controller identity or response latency.
Keep the single staging candidate open until those gates, Linux image/resource
checks and owner acceptance pass. Fuel#414 and memory#430 remain visible failures,
not silently changed coefficients, distances or fabricated finishes.

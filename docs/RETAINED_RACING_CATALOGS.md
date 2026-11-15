# Hosted Racing catalog coexistence

15 November 2026 — candidate implementation, not remote activation. Tracks#432;
part of the one interactive staging release#415/game#321.

## Explicit operator allowlist

Authority retains its existing `PITGUN_RACING_CATALOG_RELEASE_DIR` and model. Optional
`PITGUN_RACING_RETAINED_CATALOG_RELEASE_DIRS` adds immutable release directories
separated using the platform path-list convention (colon on Linux). Each snapshot
is loaded and checked against the configured model at startup. Duplicate release
identities or invalid/incompatible catalogs fail startup. Request paths/URLs never
select files or cause catalog downloads.

A requested complete catalog identity selects its exact configured snapshot.
Unknown identities still undergo primary manifest validation and fail closed.
No release is relabelled; old inputs, authorizations and primary defaults remain.

Verifier has the same optional ordinary-route allowlist plus
`PITGUN_RACING_ATTEMPT_RETAINED_CATALOG_RELEASE_DIRS` for dynamic attempts. The
corresponding model identity/trust/policy are unchanged. The execution-resolution
release selects only a configured engine; that engine still validates signature,
model/data pack, full lineage and independently calculated evidence. A mutated
manifest is rejected, not treated as a reason to load another catalog.

All engines/routes share the same AppState replay semaphore and receipt-time
admission. A catalog variant does not create capacity or change the24h window.
Service-control retains its independent exact catalog/runtime envelope and trusted
WASM replay; legacy driver permissions do not grant tyre-service actions.

## Candidate staging shape

- Authority primary model0.14/catalog1.8.0, additional1.8.1.
- Verifier ordinary primary model0.11/catalog1.6.0, additional1.8.1.
- Verifier dynamic primary model0.14/catalog1.8.0, additional1.8.1.
- Separate service-control model0.11/catalog1.8.1/runtime07f5bd5.

This supports old admitted1.8.0 careers and fresh1.8.1 preparation without globally
switching the old default. Retire any release only after its admissions have
settled/expired and pending proof recovery is accounted for.

## Executed qualification

27 Authority tests,15 Verifier library tests and4 binary tests pass, plus Clippy
with warnings denied and fmt. New cases cover issuance of FP/attempt under both
identities, real native verification of old/new evidence, substituted manifests
and exact route selection; existing receipt-time/capacity tests remain.

Actual separate Docker images plus Chrome production-compiled frontend also run
legacy WASMa36/catalog1.8.0 and interactive WASM07/catalog1.8.1 through HTTP admission,
compiled Worker execution and the same private Verifier/attempt route. Both return
VERIFIED; unknown manifest is rejected, without /src or development artifacts.
The complete career/recovery/publication recipe and image pins are recorded by the
[game release dossier](https://github.com/loicbelec/pitgun-game/blob/feat/controlled-racing-worker/docs/releases/INTERACTIVE_STAGING_PREPARATION_2026-11-15.md).

No solver, physics, model catalog bytes, browser WASM or schema changed in this
backend increment. The native WASM source remains093b1ab. Local Docker and CI do
not qualify hosted PHP/TLS or imply staging/production deployment.

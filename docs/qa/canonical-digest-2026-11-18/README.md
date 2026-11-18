# Canonical digest materialization — 18 November 2026

Follow-up to [browser completion](../browser-completion-2026-11-18/README.md),
framework430/game315. Local candidate stacked on framework437; no deployment,
normal-game admission, schema/signature change or replaced immutable artifact.

`canonical_json_digest` previously called `canonical_json_bytes`, retaining the
typed validation tree while canonical JSON was encoded into a whole-document
Vec, then hashing that Vec. The revised path:

1. Runs the **same** `serde_value` validation/traversal and releases its tree.
2. Uses the **same** canonicalizer, writing bytes into a SHA-256 writer.
3. Returns a digest only after successful serialization. Any validation or
   serialization error returns the original error, never a partial digest.

`canonical_json_bytes` still supplies identical bytes to callers that need them,
but also drops the validation tree before encoding. Strict string parsing,
duplicate-key refusal, NFC/I-JSON checks and number formatting are unchanged.
No dependency/version update. The generic contract gains no Racing semantics.

This is intentionally bounded: validation still constructs a tree, and the
canonicalizer buffers object members to sort keys. A fully streaming validator
or different canonical serializer is not introduced. Object sorting means the
SHA writer alone does not make hashing constant-memory.

## Evidence

103 contract tests and111 simulator tests pass, strict library Clippy and fmt.
A frozen pre-change implementation supplies byte/digest/error parity vectors:
nested records/enums, Unicode sorting, f32/f64, safe-integer limits, invalid map
keys, non-NFC, non-finite values and failure after partial serializer output.
Existing canonical/signature/run identities and duplicate-key checks still pass.

New WASM: `9af4de2faa031c589dea4d1e8a967a287be44ecdc411f97ea8e8c40282925fa5`.
Unchanged bridge: `9969142578e2745140c2c01d03a6dd197aefd67f112248e20898c430b7fbce02`.
New reference context/receipts; no old signature is reattributed. Same physical
Monza53/ten-car/5Hz request/seed/actions and expected evidence as local6bb2f5bf.

Three alternating cold Chrome157/M4 Pro pairs, no concurrent builds/tests:

| Worker measure | Previous metadata runtime | New digest |
| --- | ---: | ---: |
| Median elapsed |13.47s |13.55s |
| Maximum WASM linear capacity |1,339.56MiB |1,109.94MiB |
| Median sampled aggregate Chrome peak RSS |2,664.05MiB |2,508.05MiB |

No CPU/latency gain is established. Linear capacity falls229.625MiB (17.1%);
sampled RSS about5.9% in a small local series. Same complete native digest
`6227e035…657411`, observations, receipts and evidence. RSS includes shared/GPU
pages, is sampled500ms, and is neither private footprint nor guaranteed peak.
WASM linear capacity differs from physical resident RAM. No VPS/mobile/general
memory acceptance. Input loading/main WASM compilation precede the elapsed
timer; RSS includes fixture startup. No rendering/archive/career/services in
these timings. See [summary](summary.json) and [identity](digest-manifest.json).

GP53 IndexedDB interruption/resume/completed reopen keeps its journal and digest
without another controller call. [19 signed HTTP checks](signed-checks.json)
pass with disposable Authority/compact Verifier: three circuits, keep/pit/timeout,
old runtime still accepted, hostile histories/signatures/pins refused. Ephemeral
key removed. No live provider, career reward or publication in those checks.

Companion frontend removes two in-process full-result JSON round-trips. Its
separate fixed-output host experiment measures398.9->49.3ms and an identical
full outcome hash. Do not add gains across experiments. Game749 tests/lint/build
and actual short host object/reopen integration pass. Full host/Worker profiles
and reproduction commands live in game `docs/qa/RESULT_PIPELINE_2026-11-18.md`.

## Release boundaries

Keep the previous metadata runtime and all originally admitted runtime/bridge/
adapter pairs for signed histories and rollback. Admit the new artifact only
with its compatible compact Verifier and retained-version routing. The browser
still needs original-bridge identity retention for older careers. Real career
reward/publication exactly once, HQ/archive/Compare and old-weekend replay remain
gates before a coordinated staging delivery. Physics, sampling, journal,
authorization and Verifier trust semantics are unchanged.

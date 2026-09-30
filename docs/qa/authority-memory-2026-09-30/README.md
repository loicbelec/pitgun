# Authority memory correction — 30 September 2026

Status: merged; native CI and staging acceptance passed. Production promotion
awaits the owner-executed backup/isolated restore drill required by the runbook.

Framework PR #425, merge `e70dd56d7c06c6950eb23d8aeda6aaef48d2d48d`.
Image digest: `sha256:667e3def302e0f10ede0b32e95a6232b84e25811e762bcea01c578f4b410684a`.
CI runs 36762896058 (framework/WASM), 36762896183 (native memory) and
36764456586 (image publication) passed. Staging: infra-vps#95 and explicit
staging / pitgun-authority workflow 36764959308 passed. Only Authority staging
changed; 28 other containers retained their identity/image.

## Acceptance

The exact former production image reproduces OOM/137 at 256 MiB / 0.5 CPU with
eight concurrent probes. The fixed image passes 128 health probes and 66 signed
authorizations, peaking at 46.2 MiB under local AMD64 emulation and 32.0 MiB in
native Linux CI. Static/dynamic canonical authorizations match the baseline after
excluding normal issuance time and nonce; all new signatures validate against a
public fixture key. An incorrect catalogue is still rejected.

Hosted isolated Chrome completed FP1–FP3 and a 53-lap Monza race: four VERIFIED
sessions, one automatic publication, interrupted evidence recovery, save/reload
and HQ return; no browser exceptions. Gemini requests were blocked only in the
QA browser. No owner's browser or save was accessed. No new Safari/Edge coverage
or inference-quality validation is claimed.

Sixteen additional concurrent staging health/readiness checks passed. The
post-weekend memory peak was 23,916,544 bytes (22.8 MiB), with zero restarts or
OOM kills. Limits remain 256 MiB / 0.5 CPU; model 0.14.0, catalogue 1.8.0 and the
signing policy are unchanged. These are bounded checks, not a capacity claim.

`acceptance-summary.json` contains only allowlisted aggregate measurements and
booleans. Raw staging browser/session reports remain local and are not archived
in this repository. Existing baseline/fixed files use public local-test fixtures.

## Remaining production step

Infra-vps#96 prepares only the Authority image pin; Compose validation passed.
No SQL migration, frontend, Verifier, Engineer or database promotion is needed.
Keep that PR unmerged until the production preflight is complete. The runbook
requires an encrypted backup and isolated restore drill before application-image
promotion; SSH sudo requires the owner's password. The owner was asked to run
those commands. Completion must be confirmed, never inferred from elapsed time.

Then deploy only prod / pitgun-authority, verify the exact image, bounded health
and normal authorization/verification, and close framework #424 on acceptance.
If necessary, repin the old image `16a10af37554f0a005a3f83745b7336abc07f4e2`
and redeploy only Authority. A rollback needs no database restore but restores
the known memory risk; it is a fallback, not a lasting solution.

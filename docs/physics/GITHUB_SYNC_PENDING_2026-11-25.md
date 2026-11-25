# Pending GitHub metadata sync — 25 November 2026

The native qualification is complete and published in `75ee590`; [results and limits](../../experiments/thermal_candidate_qualification_v1/INTERPRETATION.md) are authoritative. Source/protocol `99ed838`, execution documentation `85aa78e`. No deployment or AI fit was performed.

## Confirmed on GitHub

- [#487](https://github.com/loicbelec/pitgun/issues/487) is closed and its project status is **Done**. The [complete result comment](https://github.com/loicbelec/pitgun/issues/487#issuecomment-7167451147) is published.
- #457 project status is **Ready**; #479 is **Validation**. These were read back from ProjectV2 after write errors, not inferred from CLI success alone.
- All source/results/documentation commits are pushed to `feat/three-compound-thermal-window`.

## Remaining operations

GitHub returned GraphQL internal errors, REST empty-JSON responses and HTTP502 during the final metadata sync. Reads confirmed that the following content did **not** publish:

1. Update #457's current-status body: both eras now pass 369 full GP / 23,352 laps, twelve exact fresh repeats and fourteen screen bridges; no new fixed-nine fit yet. Retain the older 24,966-GP campaign under its historical policy identity. Preserve its existing historical details.
2. Update draft #481's body with the expanded selected-policy qualification and its remaining cold-entry/aero/driver-risk advisories; the next gate is budget-aware fixed-nine preparation, not another coefficient refit or an admitted runtime.
3. Publish the qualification result/limits comment on #457, #479, #483 and draft #481. Their latest comments were checked and did not contain this result.

The existing #457 body can therefore still say era II is running; this is stale GitHub metadata, not ongoing computation. The two native workers have completed and their caches/evidence are retained.

Prepared payload snapshots remain outside Git in the maintainer workspace `.maintenance/thermal-candidate-qualification-2026-11-25.local/github-sync/` (`issue457-body.md`, `pr481-body.md`, `qualification-comment.md`, `pending.json`). Rebuild from the committed report if unavailable. Before retrying, fetch current bodies/comments and reconcile concurrent edits; do not overwrite newer work or duplicate comments. Clear this pending note after successful write/read-back verification.

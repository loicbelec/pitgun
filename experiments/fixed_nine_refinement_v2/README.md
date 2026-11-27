# Equal-preparation fixed-nine refinement V2

Approved follow-up to V1 REFINE (#490/#457). Preserve the 603-GP V1 archives, failed holdouts, physics and roster. This directory starts with the short Mexico diagnostic before committing the larger era-specific matrices. No candidate is admitted to the game by this research.

| Delivery | Scope / gross ceiling | Status |
|---|---|---|
| 1 | Fixed protocol, per-case durable checkpoint references, archive once | Delivered: tests and actual cache resume pass |
| 2 | Common Mexico proposals, reference/Franz/Charles, caps16/24; <=30 full GP | Complete: numerical PASS, mixed diagnostic; see PILOT_RESULTS.md |
| 3 | Era-I reachable player: three preparation families, three circuits, caps8/10/12; <=168 GP | Planned after pilot review |
| 4 | Joint nine-driver preparation on seven era-II circuits/caps12/18/24; <=858 GP | Planned; commit exact matrix before compute |
| 5 | New frozen budgets/seeds, three player references versus same field; <=120 GP | Planned after complete policy freeze |
| 6 | Native/WASM, three doctrines, recovery, control journal, performance, versioned local admission | Later gate |
| 7 | Local owner playtest, project/docs/repository administration, staging assembly | Later gate |

The ceilings are not instructions to launch all phases together. Serial eras, one native process, exact-identity reuse clearly labeled. The Mexico seed271828 is opened V1 evidence and must not be relabeled as fresh independent validation.

Pilot details: [PILOT_PROTOCOL.md](PILOT_PROTOCOL.md), [pilot-plan-v2.json](pilot-plan-v2.json), `run_pilot.py`, `harness.py`. The native executable must match the V1 hash; no rebuild or physical sampling change is needed. The shared JSON reader now closes its file handles explicitly; source identity changes require a fresh cache namespace, while the native model/result semantics stay unchanged.

Tests: `python3 -m unittest discover -s experiments/fixed_nine_refinement_v2 -v`. Run with `python3 experiments/fixed_nine_refinement_v2/run_pilot.py --binary PATH --cache PRIVATE_PARENT/mexico-pilot-v2 --output experiments/fixed_nine_refinement_v2/results/mexico-pilot-v2.json.gz` after committing sources. Use the V1 campaign's private parent directory so its native-worker lock is shared. An atomic native row precedes its hashed checkpoint record; an interrupted trailing record is retained and repaired, while complete corruption or source/data mismatches fail closed. The final compressed archive is exported once. These are orchestration improvements, not browser/WASM performance claims.

[Pilot result and limits](PILOT_RESULTS.md): 30fresh full GPs/2,130laps, no numerical/fairness failures,6exact repeats and2exact prior reference bridges. Equal-preparation elites beat the concentrated reference at both caps. The stronger cooling-wins-every-pair diagnostic fails at cap24/df0.4 and remains mixed. The named-driver old-to-new pair identifies a15.4–15.9s gain from downforce alone at cap16; this extra interpretation uses existing solves, not new reserved evaluation.

## Bounded sessions and eras III–V

The full new-physics delivery must cover **all five open eras**, with serial qualification lots: finish I/II, then III, IV, V. The current qualified native profile/probe and authored fixtures support only I/II. For each later era, export actual entry/developed career means, installed components/HQ floors, legal controls, feature unlocks and complete regular calendar before widening candidate support under a new version. Keep the same nine driver identities and retain all historical evidence. Do not claim later-era support by changing `supported_eras` alone. The era-IV KERS card currently grants game setup bonuses; physical battery/regen remains a separate explicit modeling scope.

Each era has its own finite matrix, cache, pilot, recorded outcomes, new reserved evaluation and local playable review. Plan III, IV and V as separate lots in #492; their GP ceilings are to be counted and committed from their actual datasets, not copied from the I/II168/858/120 ceilings. Keep one owner-facing local preview and enrich its stated supported scope as lots pass. Full staging/production promotion waits for I–V coverage and career-transition checks; an explicitly scoped I/II local playtest can happen earlier.

`run_pilot.py` now defaults to **20new full GPs or300seconds per invocation**, whichever limit is reached first between cases. Finish and persist an in-flight GP; the wall limit is not a hard process kill. The interval includes setup/preflight time, with checks before each pending full GP. Cache hits and already journaled results do not consume the new-GP count. The program exits normally with `complete:false`, completed/remaining counts and a stop reason; it does not chain the next batch or create a complete archive/decision. Re-run the same command to resume, optionally changing `--max-new-gp`/`--max-wall-seconds`: these are scheduling controls, not simulation inputs.

No new native GPs were launched to add/test the batching controls. Unit tests exercise count/time stops, cached interrupted work, exact reconstruction across two invocations and the partial CLI. Source identity remains strict: the previous30-GP pilot belongs to freeze `f57bdee`; use a new cache and **new results directory** for changed orchestration, rather than overwriting that historical evidence. Output from a different source/matrix is now rejected before running a GP.

## Latest execution preference: one complete era

The owner has now approved completing **all GP in one era campaign** before the next era. This supersedes the routine20-GP/5-minute invocation preference: retain those limits as optional checkpoint/recovery controls, not a requirement for owner approval every20GP. Keep one native worker and never launch multiple eras in parallel. For the existing frozen era-I audit, changing only CLI scheduling limits (`--max-new-gp 153 --max-wall-seconds 3600`) completed the remaining work without editing physical inputs or invalidating cached cases. Freeze each later era's finite matrix/counts before running the complete campaign.

Era-I attainable-player development audit is now complete:153fresh GP/9,027laps, no numerical failures,6exact fresh repeats and81checked reused rival inputs. Actual exit best free ranksP1/P1/P3; zero-extra family remainsP10across all nine contexts. [Era-I result and limits](../era1_attainability_audit_v1/RESULTS_V1.md). New reserved validation and local admission are still pending.

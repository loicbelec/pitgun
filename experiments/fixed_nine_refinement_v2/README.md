# Equal-preparation fixed-nine refinement V2

Approved follow-up to V1 REFINE (#490/#457). Preserve the 603-GP V1 archives, failed holdouts, physics and roster. This directory starts with the short Mexico diagnostic before committing the larger era-specific matrices. No candidate is admitted to the game by this research.

| Delivery | Scope / gross ceiling | Status |
|---|---|---|
| 1 | Fixed protocol, per-case durable checkpoint references, archive once | Implemented; interruption/tamper/export tests |
| 2 | Common Mexico proposals, reference/Franz/Charles, caps16/24; <=30 full GP | Frozen protocol; execute next |
| 3 | Era-I reachable player: three preparation families, three circuits, caps8/10/12; <=168 GP | Planned after pilot review |
| 4 | Joint nine-driver preparation on seven era-II circuits/caps12/18/24; <=858 GP | Planned; commit exact matrix before compute |
| 5 | New frozen budgets/seeds, three player references versus same field; <=120 GP | Planned after complete policy freeze |
| 6 | Native/WASM, three doctrines, recovery, control journal, performance, versioned local admission | Later gate |
| 7 | Local owner playtest, project/docs/repository administration, staging assembly | Later gate |

The ceilings are not instructions to launch all phases together. Serial eras, one native process, exact-identity reuse clearly labeled. The Mexico seed271828 is opened V1 evidence and must not be relabeled as fresh independent validation.

Pilot details: [PILOT_PROTOCOL.md](PILOT_PROTOCOL.md), [pilot-plan-v2.json](pilot-plan-v2.json), `run_pilot.py`, `harness.py`. The native executable must match the V1 hash; no rebuild or physical sampling change is needed. The shared JSON reader now closes its file handles explicitly; source identity changes require a fresh cache namespace, while the native model/result semantics stay unchanged.

Tests: `python3 -m unittest discover -s experiments/fixed_nine_refinement_v2 -v`. Run with `python3 experiments/fixed_nine_refinement_v2/run_pilot.py --binary PATH --cache PRIVATE_PARENT/mexico-pilot-v2 --output experiments/fixed_nine_refinement_v2/results/mexico-pilot-v2.json.gz` after committing sources. Use the V1 campaign's private parent directory so its native-worker lock is shared. An atomic native row precedes its hashed checkpoint record; an interrupted trailing record is retained and repaired, while complete corruption or source/data mismatches fail closed. The final compressed archive is exported once. These are orchestration improvements, not browser/WASM performance claims.

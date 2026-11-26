# Fresh native reserve for the frozen I/II field

This validates a fixed candidate; it never optimizes on fresh results. See
[the preregistered protocol](ACCEPTANCE.md). Retain era-I point interpolation and
era-II complete setup/stint anchors, with development-only frozen reference
recipes. No runtime, local playable preview or deployment is changed here.

Run focused tests in this directory with `python3 -m unittest -v test_reserve.py`.
`candidate.py` exactly regenerates `candidate-v1.json` from retained development
evidence. Commit/clean source before native compute. Run `run.py --era 1` then
`--era 2` with `--binary`, distinct `--cache` directories sharing the maintenance
parent/worker lock, and `--output` paths outside the repository until both finish.
Defaults allow each complete finite era, one native worker, durable checkpoints.

Plan: I105new GP, then II234new GP,339total /21,592laps, including12fresh repeats.
No full results reused; three exact id-only reference aliases are deduplicated.
The three new seeds are checked against retained development/pilot/old reserves.
Current status: source/protocol preparation, no new reserve result yet.

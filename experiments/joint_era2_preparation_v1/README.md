# Joint era-II field preparation

One shared preparation pool, one whole-era campaign: calibrate the nine fixed
drivers and compare player options together. This is not another player-only
audit against an unchanged field. See [preregistered review](ACCEPTANCE.md).

Run focused tests from this directory: `python3 -m unittest -v test_joint.py`.
Run `run.py --binary /path/to/frozen/fixed_nine_budget_probe --cache /path/to/joint-era2-v1 --output results/era2-v1.json.gz`.
Defaults allow the entire finite era, one worker,7200s soft wall limit; checkpoints
are durable after each GP. Source must be committed/clean before first compute.
Exact compatible V1 rows are imported unchanged after native resolution bridges.
Partial runs never publish a complete decision or policy.

Status: complete **development review PASS**, source/protocol `a3d11f7`.
579new full GP /37,987new laps,279exact unchanged reused rows,18/18fresh repeats.
All preregistered numerical and gameplay gates pass. The complete candidate
anchors are frozen in `results/policy-v1.json`, with admission explicitly false.
See [results and limitations](RESULTS_V1.md).

Next: consolidate the era-I/II field, freeze new reserved budgets/seeds, then
native/WASM/recovery and explicit local-game admission. III/IV/V remain separate
whole-era joint lots; full new-model staging must cover all five open eras.
The existing local intermediate game preview has **not** received this field.

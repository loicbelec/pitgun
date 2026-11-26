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

Status: protocol preparation, no era-II refit result or game admission yet.
The existing local intermediate game preview has **not** received this field.

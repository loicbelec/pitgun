# Joint era-III preparation

This follows the selected common140kg screen. It prepares the nine frozen actors
and reference player together rather than auditing an artificial player optimum
first. See [preregistered scope and gates](ACCEPTANCE.md).

`plan-v1.json` plus `run.py` defines the immutable finite matrix. The structural
budget proof is in `results/foundation-budget-v1.json`; regenerate with:

```sh
node experiments/joint_era3_preparation_v1/audit_budget.mjs GAME_CHECKOUT NEW_OUTPUT.json
```

Run the pinned existing native `era3_budget_probe` executable:

```sh
python3 experiments/joint_era3_preparation_v1/run.py \
  --binary FROZEN_BINARY --cache PRIVATE_CACHE --output PRIVATE_RESULTS.json.gz
```

The full836-GP era runs sequentially. `--max-new-gp` and `--max-wall-seconds`
allow interruption **between** GP. Rerun the same command to resume. The journal
and row files are the checkpoint; partial progress is not a final result.
An unadmitted candidate and review are emitted only after the complete archive.
Never alter pinned files or rebuild the executable during the run.

No game assets, UI, player storage or deployed services are modified here.

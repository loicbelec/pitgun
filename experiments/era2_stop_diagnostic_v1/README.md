# Era-II stop-count diagnostic

Follow-up to [the retained stop-count counterexamples](../era2_strategy_qualification_v1/STOP_COUNT_REVIEW.md), tracked in [#485](https://github.com/loicbelec/pitgun/issues/485), under [#483](https://github.com/loicbelec/pitgun/issues/483). Read [ACCEPTANCE.md](ACCEPTANCE.md) for the frozen scope, [RESULTS_V1.md](RESULTS_V1.md) for the comparisons, and [INTERPRETATION.md](INTERPRETATION.md) for the decision. This experiment does not change the game or its physical policy.

## Reproduce

Use a clean checkout containing the pinned native/runner sources recorded in the archive; their original commit is recorded in `evidence-archive-v1.json`. The read-only reporting tool and this documentation were added after execution and are available in the later evidence commit. Keep all retained resources, including the baseline archive. Pick a fresh absolute cache directory outside the checkout; reuse an existing cache only with the same pinned inputs/executable/source. The runner rejects a dirty initial checkout and changed cache identity.

```sh
python3 -m unittest discover -s experiments/era2_stop_diagnostic_v1 -v
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/ABSOLUTE/TARGET cargo test --offline --release -p pitgun-racing-simulator --example era2_stop_diagnostic_probe
python3 experiments/era2_stop_diagnostic_v1/run.py --target-dir /ABSOLUTE/TARGET --cache /ABSOLUTE/NEW-CACHE/campaign --output /ABSOLUTE/NEW-CACHE/campaign.json.gz
python3 experiments/era2_stop_diagnostic_v1/report.py /ABSOLUTE/NEW-CACHE/campaign.json.gz --output-dir /ABSOLUTE/NEW-REPORT
```

Only one simulation process runs at a time; compilation uses two jobs. The retained matrix is 44 main full GPs plus four fresh repeats. `--limit N` checkpoints a partial run; it cannot certify a finished campaign. Interrupted runs resume from individually pinned cache entries. The exclusive lock prevents concurrent invocations of the same campaign.

The runner stores complete resolved-input receipts, compound/heat parameters, full-result digests, per-lap scalars, thermal ledgers, checks and failures. It does not retain high-frequency graphical telemetry. The archive includes every main run and all four fresh repeats. The report reads this evidence without fitting coefficients; rerunning it does not simulate anything.

The example is derived from the frozen era-II qualification probe in a separate file so that the original source and archives remain intact. Its two counterfactual switches are offline diagnostics only. Neither the solver library nor any runtime selector exposes them. Do not publish their results to the leaderboard or transfer their parameters to production as calibrated values.

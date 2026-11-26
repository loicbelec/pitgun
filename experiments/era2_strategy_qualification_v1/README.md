# Era-II bounded strategy qualification

**Fuel-scope correction:** retained runs start with100kg. The110kg wording in the original acceptance/report prose is incorrect; only100kg was qualified. See [the explicit erratum](../three_compound_thermal_v1/FUEL_SCOPE_ERRATUM_V1.md).
Tracking #483 / #479 / draft #481. Same frozen three-compound candidate as era I; actual career component and bonus-floor fixtures. See [acceptance](ACCEPTANCE.md), [results](RESULTS_V1.md), [decision](DECISION_V1.json) and [archive hashes](evidence-archive-v1.json).

Run one native process at a time:

```sh
python3 experiments/era2_strategy_qualification_v1/run.py --target-dir /ABSOLUTE/CARGO/TARGET --cache /ABSOLUTE/PRIVATE/CACHE --output /ABSOLUTE/PRIVATE/campaign.json.gz
```

An exclusive lock and immutable source/binary/plan/fixture/policy pins protect resumed cases. `--limit 1` is an explicitly incomplete harness preflight. Archives retain inputs, scalar lap diagnostics and all failures, rather than full telemetry histories.

Numerical qualification passes:195 main GP and6 exact full-result repeats,201 complete GP/13158laps. **Strategic qualification remains open**: the three-stop diagnostic wins against sampled one/two-stop schedules on all seven exit-car contexts. These recipes do not establish global optima. Preserve the counterexample and diagnose full pit cost, stint timing, thermal/grip behavior before freezing opponents; do not impose a player-only stop cap or claim the physics balanced.

A separately versioned, unranked local preview may expose the candidate for owner observation. No signed runtime admission, hosted verification, AI calibration or deployment follows from this report.

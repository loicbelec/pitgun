# Conditional aero follow-up

[#489](https://github.com/loicbelec/pitgun/issues/489), following the closed [48-GP SM audit](../aero_tradeoff_audit_v1/INTERPRETATION.md). Protocol/source `ac6e809`, committed before these fifteen new races. See [ACCEPTANCE.md](ACCEPTANCE.md) for scope and [RESULTS_V1.md](RESULTS_V1.md) for the conditional comparison.

The same frozen aero probe/checker is reused without compilation or solver/resource changes. Three baseline cases match prior qualification semantically before solving and match physical-input/full-result digests afterwards. Three fresh repeats match too. Fifteen new GP /880laps pass; the original48-GP matrix is unchanged and not relabeled as a63-GP preregistered experiment.

The key finding is a sign change. Extra downforce at fixed drag area gains25–37s over the full matched-MH two-stop race, whereas it lost24–39s over the original SM-one-stop plan. Temperature still rises under extra load, but the strategy makes the tradeoff useful. The ordinary coupled slider also becomes circuit-dependent: midpoint beats minimum on Silverstone/São Paulo, while Monza favors minimum among0/.5/1. An exact optimum and all other strategies/budgets are not identified.

Use clean pinned sources, the original aero-probe binary identified in the plan and a fresh cache under the same campaign parent lock. The post-run report was added afterwards; it does not select a coefficient or alter the experimental matrix.

```sh
python3 experiments/aero_plan_followup_v1/run.py --binary /ABSOLUTE/aero_tradeoff_probe --cache /ABSOLUTE/CAMPAIGN/mh2-followup --output /ABSOLUTE/CAMPAIGN/mh2-followup.json.gz
python3 experiments/aero_plan_followup_v1/report.py /ABSOLUTE/CAMPAIGN/mh2-followup.json.gz --output-dir /ABSOLUTE/CAMPAIGN/report
```

Do not transfer an apparent aero preference from one tire plan to every race. Keep aero and compound/stint choices together in the next bounded opponent preparation; retain fixed driver identities, exact budget/bonus sums, reserved budgets/seeds and player winning latitude. The broader #469 point-value/gearing audit is not closed by these three conditional contexts. No game/catalog/runtime/Authority/Verifier or deployment change.

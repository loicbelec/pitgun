# Era-II stop-count diagnostic V1 — preregistration

Offline only. Frozen operating-policy SHA256 `0021bd8ead91698de49de297b1760cc3225a54c9e61adfeb07bab8250ab16439`; no production kernel, catalog, game, AI, admission or deployment change. Fuel starts at **100 kg**, as established by the retained native receipts and the fuel-scope erratum.

## Bounded matrix

Monza (53 laps) and São Paulo (71 laps), developed Craft era-II fixture, discretionary allocation A/C/E/K `[4,4,4,4]` plus the actual `[2,1,4,1]` bonuses, sliders 0.5, balanced reference driver, seed 73013. These are two contrasting retained counterexamples, not representative coverage of every circuit or budget.

For each circuit compare medium/hard and soft/medium families, each in both starting orders, with one, two and three stops. Preserve **exact integer lap totals per compound within each family/order**. For odd race lengths the first named compound receives floor(N/2) laps. Add early/late first-stint variants for the two-stop medium/hard family, still preserving those totals. This produces 32 nominal full GPs. Sequence, fuel history, tire age and service preparation remain coupled; this is a controlled comparison, not a complete optimum search.

Add 12 counterfactual full GPs: medium/hard forward-order one/two/three-stop recipes on both circuits, separately removing temperature-only grip loss or wear-only grip loss. Never remove both together. Preserve temperature states, wear generation, mount temperature, heat coefficients and partitions, engine/fuel laws, driver, service cost and sampling. Disable the selected multiplicative grip factor on **all** initial/replacement/thermal-lookup tire resources. Counterfactuals have explicit case and physical-input identities and cannot be admitted as production results. Changing grip changes speed, work, fuel and thermal trajectories: these are coupled sensitivity experiments, **not additive attribution of lap time**.

Total: **44 main GP +4 fresh exact repeats =48 GP**, one serial worker, no broad all-era or all-budget search. Do not automatically expand this matrix.

## Required checks

- Commit source before execution; bind binary/source, plan, baseline archive, fixtures and policy hashes. Reject mismatched cache reuse; retain failures and accepted prefixes. Keep old archives/probes immutable.
- Exactly 100 kg initial fuel, actual fixture budget/components, unchanged heat coefficients, every scheduled service charged, lap sums plus service equal finish time, exact pit preparation, continuous lap-boundary states, thermal conservation within the existing qualification tolerances.
- Four fresh repeated runs must match full result and physical-input digests, not just lap times.
- Nominal MH-half and SM-half on both circuits must reproduce the four prior archived physical-input and full-result digests. This is a bridge to the frozen experiment, not independent validation of realism.
- Native tests prove each ablation changes only its declared tire fields, preserves every heat parameter and keeps the other grip factor unchanged. Runner tests protect equal compound exposure, scope, fuel and service checks.

## Decision

Report complete service cost, per-stint wear and temperature/grip trajectory, matched schedule rankings, timing sensitivity and counterfactual differences. A one/two-stop victory is **not** a pass requirement. Do not invent a player-only stop limit, raise pit losses or retune tires to manufacture that result. If three-stop counterexamples persist, leave the strategic gate open and propose the next versioned hypothesis before final nine-AI fitting. Numerical PASS alone is not physical calibration or industrial validation.

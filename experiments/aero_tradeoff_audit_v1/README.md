# Aerodynamic tradeoff audit

Focused follow-up [#488](https://github.com/loicbelec/pitgun/issues/488) under #469, after [selected thermal qualification](../thermal_candidate_qualification_v1/INTERPRETATION.md). Read [ACCEPTANCE.md](ACCEPTANCE.md), [RESULTS_V1.md](RESULTS_V1.md) and [INTERPRETATION.md](INTERPRETATION.md). The Solver, current game and admitted policy are unchanged.

## Resolved mapping and reference convention

With `u` the slider, `p` effective aero points divided by20, and catalog reference areas `Lref`/`Dref`, the current reduced mapping is:

```
Lsetup = Lref (0.75 + 0.5 u)
AL = Lsetup (1 + 0.08 p)
AD = [0.85 Dref + 0.25 (Lsetup - 0.75 Lref)^2] (1 - 0.04 p) + Dcooling
Fdownforce = 0.5 rho v^2 AL
Fdrag = 0.5 rho v^2 AD
```

`AL` and `AD` are coefficient-times-reference-area quantities in m², not dimensionless coefficients; .25 has units1/m². The current developed basic-aero fixture has Lref4m², Dref.9m² and six effective aero points. At sliders0/.5/1 it resolves to downforce areas3.072/4.096/5.120m². Slider0 therefore retains75% of the same-car midpoint downforce; it is not zero lift.

The generic finite-wing relation links induced drag to squared lift coefficient and requires compatible reference areas ([NASA induced drag](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/induced-drag-coefficient/), [NASA drag equation](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/drag-equation/)). These sources do not identify racing-car coefficients or the present reduced polar. Converting a dimensionless quadratic to coefficient-area variables introduces a reference-area factor. Pitgun's implemented quadratic is additionally **shifted to the minimum-slider reference**, rather than proportional to total downforce squared. Treat its baseline/minimum-drag convention explicitly; neither changing it blindly to an unshifted square nor forcing a chosen optimum is justified by these references alone.

Downforce enters the normal load, load-sensitive tire capacity and remaining longitudinal force circle, including braking. It also increases rolling work and affects coupled tire heat/wear. Holding drag area in a control does not hold aerodynamic drag force/work constant when the speed changes, and does not hold rolling/deformation losses constant.

## Evidence and reproduction

Use committed source `a3818da`, or a clean checkout with the identical pinned sources, a fresh absolute cache outside the checkout and a shared target directory. A one-case pilot was checked and resumed from its unchanged cache; it is included once in the42 primary GP, not counted twice.

```sh
python3 -m unittest discover -s experiments/aero_tradeoff_audit_v1 -v
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/ABSOLUTE/TARGET cargo test --offline --release -p pitgun-racing-simulator --example aero_tradeoff_probe
python3 experiments/aero_tradeoff_audit_v1/run.py --target-dir /ABSOLUTE/TARGET --cache /ABSOLUTE/CAMPAIGN/campaign --output /ABSOLUTE/CAMPAIGN/campaign.json.gz
python3 experiments/aero_tradeoff_audit_v1/report.py /ABSOLUTE/CAMPAIGN/campaign.json.gz --output-dir /ABSOLUTE/CAMPAIGN/report
```

`--limit N` retains a partial checkpoint and cannot certify a finished audit. One native process, two compiler jobs, exclusive campaign lock, source/executable/resource hashes and retained case failures. The frozen base harness caches its original checks; the audit wrapper recalculates its additional bin/coefficient checks on every fresh or cached response and retains them in the campaign archive. Completed reports recompute those checks and all repeats/bridges independently.

The V1 receipt field `reference_mechanical_drag_downforce_and_legacy_drag_xy` contains **mechanical drag, mechanical downforce, legacy vehicle drag-X, legacy vehicle drag-Z**, in that order; its `xy` suffix is a naming typo, not a Y-axis model. The thermal resolver changes mechanical cooling drag separately from legacy vehicle fields. Coupled controls preserve the original representation and exact digests rather than silently normalizing it.

Only scalar per-lap/bin observations are archived, not graphical telemetry. The exact native executable is retained separately outside rebuildable Cargo output and identified by hash. No provider, Authority/Verifier, leaderboard or deployed service is called.

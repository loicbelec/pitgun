# Aero audit findings

## The downforce contribution is present

All48 full GP /2,796 laps pass numerical, conservation and coefficient-binding checks. Six fresh repeats and nine prior qualification input/full-result bridges match exactly. No physical or mapping coefficient changed.

Removing downforce at the .5 reference drag area costs **294.850 s at Monza, 423.103 s at Silverstone and 448.120 s at São Paulo**. This synthetic comparison holds drag coefficients, not a realistic complete wing-removal geometry. It nevertheless demonstrates that the existing tire/corner/braking chain uses downforce materially. Slider0 preserves75% of midpoint downforce and is completely different from this diagnostic.

## Immediate benefit versus full-race response

Increase downforce from slider.5 to.8 while holding aerodynamic drag area at its .5 reference:

| Circuit | First-five driving delta | Full-GP delta | Mean surface temperature delta | Mean temperature-grip delta |
|---|---:|---:|---:|---:|
| Monza | −5.936 s | +35.415 s | +4.447°C | −.0370 |
| Silverstone | −7.007 s | +23.889 s | +3.854°C | −.0298 |
| São Paulo | −4.702 s | +38.756 s | +3.774°C | −.0308 |

The short prefix benefits from more downforce, but that advantage reverses over this full SM-one-stop plan. Tire state is live: normal load, rolling/deformation work, heating, thermally accelerated wear and fuel/speed history respond. The observed hotter tires/lower thermal grip are consistent with an important coupled thermal cost; this experiment does **not** assign every lost second to temperature alone. Aerodynamic drag **area** is held, not the force/work along a changed trajectory, nor the other mechanical losses.

Among the five ordinary coupled settings, .2 leads the first-five prefix on all three circuits, while0 leads the full GP. The model therefore already has a useful short-session tradeoff. The full-race low-slider preference cannot be explained simply as missing downforce benefit. Increasing the slider also incurs a substantial drag-area increase under the present shifted quadratic.

No coupled run spends time at ≥98% nominal redline in top gear. This excludes a persistent hard top-gear redline ceiling in these observations, not every gearing-efficiency limitation. Mechanical brake-cap exposure is recorded but does not erase the observed initial downforce benefit. Fixed-curvature time partitions are comparable; braking-phase durations are not fixed spatial sectors and their differences must not be added as independent braking seconds.

## What is not established

This audit holds one car/budget, gear.5 and **SM one stop**, because those settings reproduce the earlier aero contrasts exactly. That SM plan is not the best developed-car strategy in the preceding qualification: medium/hard and shorter stints are competitive. Extrapolating the apparent aero optimum to an adapted tire plan, different engine/driver, all budgets or all hardware would overstate the result. No true global endpoint optimum is established by five sampled positions.

The low-slider reference already has substantial absolute downforce, and the polar's minimum-drag convention/coefficients remain own-model hypotheses. The generic finite-wing literature clarifies units and convention; it does not supply a measured correction for this vehicle. Do not immediately change the aero coefficients, widen the range or impose a minimum slider solely to move the winning setting.

## Proposed bounded follow-up before fine AI settings

Keep this48-GP matrix closed and immutable. A **separate preregistered 15-GP maximum follow-up** can test the same three contexts on the previously competitive matched-MH two-stop plan: coupled0/.5/1 and downforce-only.8 (12 new primary runs), plus three fresh .5 repeats. The three ordinary .5 cases can bridge to prior qualification. This changes the overall strategy (compounds and stint/service timing), not a pure material-only ablation.

If the downforce benefit survives over that adapted full-race plan, strategy/thermal interaction explains much of the original apparent optimum. If the low range still dominates and added downforce remains inefficient, investigate the basic-hardware reference areas and polar convention with separately versioned, justified hypotheses before final coefficient selection. Do not use an AI pace multiplier or repeat a large optimization campaign to hide the behavior.

The current selected thermal qualification remains its own result. No game/local preview, catalog, saved career, admitted model, Authority/Verifier or deployment changes here. Nine-AI budget preparation may proceed as planned; finely freezing its aero choices should respect the remaining #469 evidence.

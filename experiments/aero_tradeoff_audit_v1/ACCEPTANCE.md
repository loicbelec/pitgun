# Aero tradeoff audit V1 — preregistration

Under #469, before freezing the refreshed nine-AI field. Diagnose the currently qualified `rolling-heat-065` policy; **no solver, mapping coefficient, runtime, catalog or game change**. No requirement to manufacture an interior optimum or a different winner on every track.

## Fixed scope

Three developed Craft era-II contexts: Monza53, Silverstone52, São Paulo71 laps. Discretionary A/C/E/K `[4,4,4,4]`, actual bonus floors `[2,1,4,1]`, V8_1970/basic aero, gear .5, balanced reference driver, seed73013, fuel100kg and SM one stop at floor(N/2). All native inputs use the same retained career resolver and operating policy. This is a conditional audit, not all budgets/components/drivers or an optimized strategy search.

Per context:

- Five ordinary coupled slider values: 0, .2, .5, .8, 1.
- Four drag-only coefficient controls at 0/.2/.8/1: keep the ordinary slider's drag, hold downforce at its .5 reference.
- Four downforce-only coefficient controls at 0/.2/.8/1: keep the ordinary slider's downforce, hold drag at its .5 reference.
- One explicitly synthetic zero-downforce control at .5 reference drag. This is not a new game option or realistic removal of the complete wing/body assembly.

**42 primary +6 fresh repeat GP =48 maximum**, one native process at a time, two compilation jobs. Fresh repeats: coupled .5 on all three circuits, plus Silverstone drag-only .8, downforce-only .8 and zero-downforce. No post-hoc expansion or retune. Prior qualification archives remain immutable.

## Observation and causal limits

Retain full-race and first-five-lap driving totals, actual drag/brake work, tire state and per-lap spatial-bin observations. Fixed bins use absolute supplied track curvature: ≤.001 rad/m, (.001,.003) rad/m and ≥.003 rad/m. These are geometry classes, not official circuit sectors or hand-picked braking markers. Their time and distance partitions must close exactly within declared tolerance. Actual energy-path brake force supplies braking duration/work and mechanical-cap exposure. A changed braking phase is not a fixed spatial sector; do not add its duration difference as independent causal seconds.

Bin drag work is reconstructed from accepted midpoint speed and mechanical drag area, and checked against the native energy ledger. Brake work uses the accepted actual force channel (historically named brake_force_budget_n). Per-lap time/distance residuals ≤1e-7, drag/brake work residuals ≤1e-3 J. Record top-gear near-redline time (≥98% nominal redline), speed, normal load and lateral fraction of available tire force as diagnostics. Held gear .5 and driver limits can condition the apparent optimum.

Coefficient controls change only their declared aerodynamic fields. Thermal/wear/fuel/driver states then respond normally; these are **coupled trajectories**, not additive decomposition or frozen-temperature lap-time derivatives. The native thermal resolver adjusts mechanical cooling drag separately from legacy vehicle drag fields. Preserve that representation in coupled baselines; never silently normalize old inputs. Counterfactuals update relevant mechanical and legacy fields consistently with their separate references. No extra braking energy, clamped temperatures, fake throttle or adjusted service loss.

## Gates and outcome

- Source/protocol/plan/policy/fixture/executable/archive hashes before execution; clean committed initial sources, immutable phase cache, retain failures and accepted prefixes.
- Existing numerical fuel/budget/component/continuity/service/thermal checks plus bin conservation and coefficient binding.
- Nine ordinary coupled .2/.5/.8 cases bridge exactly to prior qualification input/full-result digests; all six fresh repeats must match both digests.
- Explain the full five-point response, drag-only/downforce-only/zero-downforce sensitivities and curvature/braking/thermal/gearing conditions. An inconclusive result is allowed. Numerical PASS is not proof of real aerodynamic coefficients.
- Retain source mapping units and the distinction between minimum-drag reference and zero lift. Propose a separately versioned next hypothesis only if the evidence supports it. No coefficient tuning or repeat of the large AI campaign in this audit.

# Racing Parameter Inventory V1

Status: governed analysis for issue #243. This inventory changes no production
behavior. It records the values that currently shape Racing execution, their
ownership, and the evidence required before any replacement is published.

The equation structures that consume these values, including the Game Model
and future Reference Model boundary, are reviewed in the
[Racing Model Approximation Audit V1](RACING_MODEL_APPROXIMATION_AUDIT_V1.md).

## Scope and lineage

The reviewed production boundary is Racing model `pitgun.racing@2.0.0` with
Racing Catalog `v1.3.0`. The source review covers:

- `pitgun-racing-solver` equations and defaults;
- `pitgun-racing-simulator` catalog parsing and race orchestration;
- `pitgun-racing-policy` input quantization and budget validation;
- the immutable `catalogs/racing/v1.3.0/simulation` resources;
- the historical vehicle candidates still present in the Python tooling and
  framework `dev` lineage.

Large circuit vectors and torque curves are identified by immutable catalog
resource rather than copied into this document. Scalar constants, bounds,
defaults, and response coefficients are listed explicitly.

## Classification

| Class | Correct owner |
| --- | --- |
| Equation or algorithm invariant | versioned Rust model identity |
| Universal physical constant | compiled code with unit and source |
| Vehicle, circuit, tire, driver, or power-unit data | immutable Racing Catalog resource |
| Calibrable model coefficient | immutable model resource interpreted by a compatible Rust model |
| Numerical implementation detail | compiled code and convergence or stability test |
| Gameplay or opponent policy | immutable policy resource, outside physical equations |
| Execution input | canonical run contract |

`Candidate` below means a value may move into a future catalog model resource.
It does not mean the current value is approved or that a player may choose it.

## Critical findings

### P0 — progression is silently saturated

The gameplay policy accepts `0..100` points on each development axis, but the
Solver independently clamps every axis to `0..20`. Values 20 through 100 are
therefore physically identical on an axis even though the game may continue to
sell and display progression. This reconciles with issue #229 and must be fixed
before a multi-era economy or opponent policy is trusted.

### P0 — enabled-era vehicle contracts diverged at inventory time

The reviewed game named `classic_v6t_1980`, `classic_v10_1990`, and
`classic_v8_2000`, while Catalog 1.3.0 did not publish them. The generic Racing
contract separately mapped Eras 3 and 4 to one `GroundEffect1970` class. Issue
#248 resolves this inventory finding through the executable
[Racing Game Vehicle Contract V1](RACING_GAME_VEHICLE_CONTRACT_V1.md): the
unreviewed identifiers are rejected and Eras 3–4 retain the governed 1970 base.

### P0 — historical vehicle candidates are aliases or partial variants

The recoverable `classic_v6t_1980` candidate resolves to the exact components
used by current `modern_v6t`. The V10 and V8 candidates use the default chassis,
medium tire, and an aerodynamic resource whose bytes are identical to current
`basic`; only their engine resources differ. Restoring the names unchanged
would fill a catalog lookup gap but would not establish three distinct physical
generations.

### P1 — several accepted inputs are dead or weakly identifiable

- `n_upshift` and `n_downshift` are parsed for every engine but never used;
- the public `hz` request is not used by race execution, which emits player
  telemetry at a fixed 5 Hz;
- cooling development is inert unless engine temperature crosses `t_soft`;
- downforce and drag development share one multiplicative `aero_k`;
- all gearbox ratios move together, so individual ratio design is absent;
- player points are canonicalized to whole points and rounded again before the
  Solver, producing expected sub-point dead zones.

### P1 — race assumptions hide circuit and power-unit differences

- every non-default published physical circuit omits `pit_loss_ms` and falls
  back to the same 22,000 ms loss;
- every race starts with 100 kg of fuel and 90 °C tire temperature;
- missing engine fuel burn silently becomes `0.02 kg/s`; the current
  `v6t_hybrid` resource relies on this fallback;
- fuel changes mass but is not connected to an energy balance or fuel-flow
  limit;
- an `active` aero resource is still a static pair of drag/downforce states.

### P1 — the configured speed cap has an ambiguous unit

`SimConfig.max_speed` is set to `400.0` while Solver velocity is in metres per
second. The actual numerical cap is therefore 400 m/s, or 1,440 km/h. The
separate experiment guardrail is 400 km/h. Documentation must not call the
runtime value the “Solver's 400 km/h cap”.

## Production tuning response

All fields below currently live in `TuningResponseV1::default()`. Validation
only checks finiteness and the broad inequalities shown; it does not validate
physical plausibility.

| Field | Current value | Unit | Applied meaning | Current validation | Classification and evidence |
| --- | ---: | --- | --- | --- | --- |
| `development_points_cap` | 20.0 | points/axis | clamps each of four point inputs before normalizing the response | `> 0` | Candidate; known saturation, #229 |
| `aero_development_gain` | 0.10 | ratio at full cap | scales drag and downforce areas together from `1.0` to `1.10` | `>= 0` | Candidate; coupled and early marginal effect is harmful |
| `drag_base` | 0.85 | ratio | minimum slider multiplier on drag area | `> 0` | Candidate; historical production value |
| `drag_slider_gain` | 0.30 | ratio | adds up to `0.30` drag multiplier over slider `0..1` | `>= 0` | Candidate; historical production value |
| `downforce_base` | 0.75 | ratio | minimum slider multiplier on downforce area | `> 0` | Candidate; historical production value |
| `downforce_slider_gain` | 0.55 | ratio | adds up to `0.55` downforce multiplier over slider `0..1` | `>= 0` | Candidate; historical response saturates at high downforce |
| `straight_aero_scale` | 0.95 | ratio | scales both drag and downforce in the straight state | `> 0` | Candidate; not independently identified |
| `corner_aero_scale` | 1.05 | ratio | scales both drag and downforce in the corner state | `> 0` | Candidate; not independently identified |
| `chassis_grip_development_gain` | 0.08 | ratio at full cap | scales chassis `mu0` from `1.0` to `1.08` | `>= 0` | Candidate; strongly dominant in early marginal campaign |
| `cooling_base` | 0.75 | ratio | minimum multiplier on `p_cool0` and `k_cool` | `> 0` | Candidate; no measured pace effect without derating |
| `cooling_development_gain` | 0.50 | ratio at full cap | raises cooling multiplier from `0.75` to `1.25` | `>= 0` | Candidate; activation domain not established |
| `engine_torque_development_gain` | 0.01 | ratio at full cap | uniformly scales the complete torque curve up to 1% | `>= 0` | Candidate; weak early marginal effect |
| `gear_ratio_base` | 1.10 | ratio | scales every catalog gear ratio at slider zero | `> 0` | Candidate; whole-gearbox approximation |
| `gear_ratio_slider_reduction` | 0.20 | ratio | lowers common gear multiplier from `1.10` to `0.90` | `>= 0`, `< base` | Candidate; real but weak response |

The reviewed offline aerodynamic candidate uses `drag_base=0.650`,
`drag_slider_gain=0.950`, `downforce_base=1.05625`, and
`downforce_slider_gain=0.375`. It produced useful circuit-dependent optima and
was used during model-V2 exploration, but it was never published as the
production tuning response. The Databricks review returned `REFINE`; the values
remain experimental evidence, not current defaults.

## Solver algorithm and safety values

These values are compiled into the model. A safety epsilon may stay compiled;
a physical guard or response boundary requires evidence and a model-version
decision.

| Value or rule | Current value | Unit | Classification | Status |
| --- | ---: | --- | --- | --- |
| continuous aero full-straight curvature | 0.0 | rad/m | model coefficient defining V2 | reviewed and published in model V2 |
| continuous aero full-corner curvature | 0.001 | rad/m | model coefficient defining V2 | reviewed over seven circuits |
| diagnostic straight/corner threshold | 0.001 | rad/m | diagnostic contract | does not alter V2 force response |
| near-max-RPM diagnostic threshold | 0.98 | ratio of `n_max` | diagnostic contract | observed inactive in response campaign |
| longitudinal diagnostic threshold | 0.05 | m/s² | diagnostic contract | descriptive only |
| near-zero curvature shortcut | `1e-5` | rad/m | numerical/algorithm detail | bypasses corner fixed-point loop |
| corner-speed initial guess | 70.0 | m/s | numerical implementation detail | fixed, no convergence criterion |
| corner-speed iterations | 5 | iterations/sample | numerical implementation detail | convergence not demonstrated across all resources |
| corner-speed squared floor | 0.1 | m²/s² | numerical safety detail | compiled |
| backward braking cap | `6 × g` | m/s² | physical/numerical guard | uncalibrated and candidate for named model parameter |
| integration safe speed | 1.0 | m/s | numerical safety detail | compiled |
| engine-force speed floor | 10.0 | m/s | numerical regularization | shapes low-speed acceleration; not audited |
| mass and heat-capacity divisor floor | `1e-9` | respective unit | numerical safety detail | compiled |
| acceleration distance floor | `1e-3` | m | numerical safety detail | compiled |
| thermal power floor after derating | 0.20 | ratio | calibrable physical guard | compiled for historical models and V3 profiles V1–V7; explicit in offline thermal profile V8 |
| tire temperature sigma floor | `1e-3` | °C | numerical safety detail | compiled |
| tire wear bounds | `0..1` | ratio | state invariant | compiled |
| tire temperature floor | 0.0 | °C | state guard | numerically safe but not physically sufficient |
| minimum adjusted lap time | 0.1 | s | numerical safety detail | compiled |
| telemetry throttle clamp | `0..1.2` | ratio | telemetry projection | permits 120%; should not be treated as physical throttle |
| telemetry smoothing window | 5 | samples | presentation algorithm | fixed and separate from physical execution |

The cubic smoothstep equation, backward/forward spatial passes, grip-circle
form, deterministic noise algorithm, and unit conversions are equation or
algorithm invariants. Changing them requires a new model identity, not only a
catalog release.

## Race and execution defaults

| Field or rule | Current value | Unit | Current bound | Correct owner / decision |
| --- | ---: | --- | --- | --- |
| initial fuel mass | 100.0 | kg | clamped to `>= 0` during burn | vehicle/race ruleset data; must not remain universal |
| initial tire wear | 0.0 | ratio | state later capped at 1 | execution state |
| initial tire temperature | 90.0 | °C | later floored at 0 | tire/race/environment data |
| initial engine temperature | engine `t_init` | °C | finite catalog value | engine resource |
| tire ambient temperature | 35.0 | °C | no explicit validation | event/environment resource |
| Solver default pit loss | 20.0 | s | non-negative use | compatibility default only |
| Simulator circuit fallback pit loss | 22,000 | ms | override forced to at least 1,000 ms | circuit/ruleset resource; currently universal in practice |
| maximum speed | 400.0 | m/s | no physical unit validation | execution safety bound; rename with unit |
| minimum lap count | 1 | lap | input policy allows at most 100 | execution contract |
| player telemetry rate | 5.0 | Hz | resampler floors denominator at `1e-6` | presentation/execution config, currently hard-coded |
| telemetry batch size | 64 | frames | positive compile-time value | transport detail |
| missing gravity | 9.81 | m/s² | optional catalog fallback | require or document as universal constant |
| missing engine fuel burn | 0.02 | kg/s | no plausibility bound | remove silent fallback; require explicit power-unit data |
| missing shift thresholds | 0.0 | rpm | parsed, unused | remove until implemented or activate under new model identity |
| minimum parsed gear count | 2 | gears | parser coerces upward | reject invalid catalog data instead of silently rewriting it |

`pit_time_penalty_s` advances the Solver telemetry timeline after a stop. Race
lap times receive the corresponding loss once in Simulator projection; the
current implementation does not double-count the loss.

## Driver response

Driver `aggressiveness` is catalog data clamped to `0..1`, but its physical
response is compiled:

| Derived effect | Aggressiveness 0 | Aggressiveness 1 | Unit | Assessment |
| --- | ---: | ---: | --- | --- |
| base tire-wear multiplier | 0.92 | 1.18 | ratio | calibrable, not validated against operational data |
| lap noise standard deviation | 20 | 80 | ms | deterministic stochastic model; small relative to current setup effects |
| peak pace adjustment | -20 | -90 | ms/lap | gameplay-like pace coefficient inside Solver |

Noise is drawn deterministically from `(seed, driver_id, lap)` with Box–Muller.
For one seed, driver, and lap, it cancels when comparing setups. The random
algorithm belongs to the model identity; the three response ranges are
calibrable coefficients and should not remain anonymous literals.

Current physical driver aggressiveness values are:

| Driver resource | Aggressiveness |
| --- | ---: |
| `goat_tifi` | 0.10 |
| `battery_voltas` | 0.20 |
| `default` | 0.50 |
| `smooth_operator` | 0.50 |
| `pedro_gaseoso` | 0.51 |
| `daniel_enchantier` | 0.55 |
| `luis_amilton` | 0.60 |
| `isa_kadjar` | 0.65 |
| `charles_leclair` | 0.84 |
| `franz_hermann` | 0.85 |

The `aggressive`, `balanced`, and `conservative` JSON resources contain no
aggressiveness and are ignored by the physical driver parser. They must not be
counted as physical driver profiles.

## Canonical input policy

| Input | Policy range | Quantization | Solver interpretation | Finding |
| --- | --- | --- | --- | --- |
| four development axes | `0..100` each | step 1.0 | rounded integer, then clamped to `0..20` | silent saturation |
| downforce slider | `0..1` | step 0.01 | clamped `0..1` | physical but historical optimum often saturated |
| gear-ratio slider | `0..1` | step 0.01 | clamped `0..1` | physical but weakly discriminating |
| total development | sum `<= floor(budget_cap)` | whole budget level | no era-specific physical mapping | gameplay constraint, not physical equivalence |
| laps | `1..100` | integer | minimum one in Solver | contract guard |

An equal `budget_cap` does not imply equal physical potential while point
marginals and saturation differ. Opponent Policy V3 must sample reviewed
physical frontiers, not only equal point totals.

## Current Catalog 1.3.0 scalar data

### Aerodynamics

`cdA` and `clA` are effective area terms used directly in
`0.5 × rho × v² × area`. Their expected unit is m², but the JSON schema does
not encode the unit.

| Resource | `cdA_x` | `cdA_z` | `clA_x` | `clA_z` | Finding |
| --- | ---: | ---: | ---: | ---: | --- |
| `none` | 0.30 | 0.30 | 0.00 | 0.00 | low-aero historical placeholder |
| `basic` | 0.90 | 0.90 | 4.00 | 4.00 | no straight/corner distinction before tuning scales |
| `active` | 0.80 | 1.00 | 2.60 | 4.13 | two static states, not commanded active aero |

### Chassis

| Resource | Empty mass | Wheel radius | Base grip `mu0` | Rolling resistance | Air density | Gravity |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `default` | 800 kg | 0.34 m | 1.50 | 0.020 | 1.225 kg/m³ | 9.81 m/s² |
| `f1_2026` | 768 kg | 0.36 m | 1.80 | 0.015 | 1.225 kg/m³ | 9.81 m/s² |

Air density and gravity are environmental constants stored inside each chassis,
which prevents event-level weather or planetary conditions without replacing
the vehicle resource. Their long-term owner should be environment/mission data.

### Engines

The sample magnitudes and `torque × rpm × π / 30` formula imply kN·m so
that the result is kW, but the JSON field is named only `trq_segments`; the
unit is implicit rather than contractual.

| Resource | RPM range/step | Gears | First/last total ratio | Fuel burn | Thermal derate slope | Finding |
| --- | --- | ---: | --- | ---: | ---: | --- |
| `v8_1960` | 0–11,000 / 250 | 5 | 13.0 / 5.5 | 0.02 kg/s | 0.004 /°C | published, hand-authored provenance |
| `v8_1970` | 0–13,000 / 250 | 5 | 13.0 / 4.5 | 0.02 kg/s | 0.006 /°C | published, hand-authored provenance |
| `v6t` | 0–12,000 / 250 | 5 | 14.0 / 4.5 | 0.02 kg/s | 0.010 /°C | published, used by `modern_v6t` |
| `v6t_hybrid` | 0–15,000 / 250 | 8 | 14.0 / 4.5 | fallback 0.02 kg/s | 0.020 /°C | no electrical state; silent fuel fallback |

Every published engine currently shares `t_amb=35 °C`, `t_init=90 °C`,
`c_th=100,000 J/°C`, `alpha_heat=0.45`, `p_cool0=0`, `k_cool=45`, and
`t_soft=110 °C`. Only `beta_derate` differs. This strong duplication suggests
authored defaults rather than independently calibrated power-unit thermal data.
The offline V8 thermal profile now exposes bounded relative multipliers for
capacity, heat generation, static and speed cooling, threshold, derating slope,
minimum power, response shape, and cooling drag. It preserves these engine
resources as era-aware references and authorizes no production calibration.

### Tires

| Resource | Grip scale | Base wear/s | Load wear | Wear grip slope | Minimum wear grip | Optimum temp | Sigma | Minimum temp grip | Heat | Cooling |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `hard` | 0.97 | `7e-7` | `7e-8` | 0.47 | 0.52 | 100 °C | 55 °C | 0.87 | 0.0058 | 0.00092 |
| `medium` | 1.00 | `1e-6` | `1e-7` | 0.50 | 0.50 | 100 °C | 45 °C | 0.85 | 0.0060 | 0.00090 |
| `soft` | 1.02 | `2e-6` | `2e-7` | 0.60 | 0.45 | 100 °C | 42.5 °C | 0.825 | 0.0061 | 0.00089 |

The tire heat and wear equations use a squared-acceleration load metric. Their
coefficients are operational model coefficients with implicit compound units,
not universal tire properties. They need dimensional documentation before
external calibration.

### Circuits and pit loss

The 24 named circuit resources own their full `s/x/y/z`, curvature, slope, and
heading vectors. Those immutable bytes are the current value. Their data
provenance is governed separately by the track reports.

None of the 24 named simulation resources declares a circuit-specific
`pit_loss_ms`; all currently resolve to the compiled 22-second fallback. The
`default` compact circuit explicitly carries the same 22 seconds. This makes
pit strategy insufficiently circuit-dependent even though tire wear and lap
count can differ.

## Historical Era 3–4 candidates

The candidates originated in tooling commit
`753fc5233ddb8150f39614e4d3d130ea2e2ab6f6` and were copied to framework commit
`d81e5d282326185303456b664311784bad109e2c`, which remains on the `dev` lineage
and is not an ancestor of `main`.

| Candidate | Components | Physical differentiation | Provenance verdict |
| --- | --- | --- | --- |
| `classic_v6t_1980` | `v6t` + `basic` + `default` + `medium` | exact alias of current `modern_v6t` | reject unchanged |
| `classic_v10_1990` | `v10_1990` + `modern` + `default` + `medium` | engine only; `modern` aero bytes equal current `basic` | retain as unreviewed experiment candidate |
| `classic_v8_2000` | `v8_2000` + `modern` + `default` + `medium` | engine only; `modern` aero bytes equal current `basic` | retain as unreviewed experiment candidate |

The V10 candidate uses 0–16,000 rpm, six gears, 14.5/5.0 endpoint ratios, and
`beta_derate=0.006`. The V8 candidate uses 0–17,500 rpm, seven gears, 15.0/5.5
endpoint ratios, and the same derate slope. Both reuse the generic thermal and
fuel values above. No source or calibration evidence accompanies their
historical naming.

These files are recoverable evidence, not publishable resources. Issue #248
must either design reviewed era resources or simplify the game mapping.

## Ownership decisions

### Remain compiled under a model identity

- force-balance and grip-circle equations;
- backward/forward spatial solution and time integration;
- continuous curvature smoothstep equation for model V2;
- deterministic RNG and stream derivation;
- numerical epsilons with stability tests;
- canonical unit conversions.

### Move to a future immutable model resource

- the validated development and setup response;
- named braking and derating physical guards if retained;
- driver aggressiveness response ranges;
- future energy efficiency, storage, loss, recovery, thermal, and reserve
  coefficients.

### Require as physical catalog or event data

- every enabled-era vehicle and its component identities;
- explicit fuel burn until energy accounting replaces it;
- circuit-specific pit loss;
- initial fuel load and environmental temperature;
- documented units for torque and tire operational coefficients.

### Remove, reject, or activate deliberately

- unused shift thresholds;
- ignored public telemetry-rate input;
- silent engine fuel and gear-count fallbacks;
- unresolved game vehicle identifiers;
- labels such as `hybrid` or `active` without corresponding model state.

## Consequences for the next campaign

The decision-surface campaign in #244 must not sweep every value in this
inventory. Its first governed design should isolate:

1. development-axis marginal effects at early, middle, and late budgets;
2. pairwise interactions and the current 20-point saturation boundary;
3. downforce and gearing response across representative circuit classes;
4. thermal activation conditions that make cooling observable;
5. tire and pit-strategy response with explicit per-circuit pit loss scenarios;
6. reviewed existing vehicles plus clearly labelled historical candidates;
7. held-out circuits and multiple seeds after local screening.

Only identifiable parameters with useful activation domains may advance to a
catalog proposal. Databricks records the governed evidence; it does not decide
which physical model Pitgun publishes.

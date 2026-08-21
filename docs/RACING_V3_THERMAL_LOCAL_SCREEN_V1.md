# Racing Model V3 adaptive thermal screen V1

Status: governed offline evidence for #287 and #291. It authorizes no game,
catalog, hosted-verification, or opponent-policy change.

## Campaign

The campaign binds every execution to
`pitgun.racing-v3-candidate@0.10.0` and the V8 thermal profile. The canonical
local result contains:

- 15,192 deterministic simulations;
- 263,196 simulated laps;
- six progression anchors covering enabled eras 1 through 5;
- all four enabled physical vehicle resources;
- six circuit archetypes, split into calibration and held-out partitions;
- short and long activation runs;
- 64 broad multi-parameter samples and 32 local refinement samples;
- three cooling-development levels, with the neutral level replayed on a
  second seed.

The stored report and SHA-256 digest live under
`experiments/racing_v3_thermal/results/`.

## Findings

Five controls are independently observable at the V8 baseline:

- thermal capacity;
- heat generation;
- speed-dependent cooling;
- soft-limit offset;
- cooling drag cost.

Derating slope, retained-power floor, and response shape are inactive in
one-factor baseline tests because the neutral engine rarely crosses the soft
limit. The adaptive dependency closure correctly retains them for joint
sampling. Static cooling remains exactly inactive because every current engine
resource declares a zero static-cooling reference: multiplying zero cannot
create a physical effect.

The broad 64-point surface divides into 36 healthy points and 28 pathological
points, but only seven healthy points produce the diagnostic thermal excursion
required for deeper review. Refinement around both sides of this transition
adds eight engaged healthy points. Across both stages the final classification
is:

- 15 thermally engaged, non-pathological parameter sets;
- 36 thermally disengaged parameter sets;
- 45 parameter sets with at least one execution above 180 °C or above 50%
  derated duration.

Cooling from zero to maximum development changes median temperature by between
approximately `-51.44 °C` and `-0.01 °C`, depending on the parameter set. Its
median time effect ranges from a `121.6 s` benefit in severe thermal cases to a
`12.3 s` cost when explicit cooling drag dominates. The intended physical
trade-off is therefore observable, but its useful region is narrow.

## Interpretation

The surface is thresholded rather than smoothly calibrable over the complete
reviewed range. Minimizing temperature alone would select engines that remain
near the `90 °C` initial state and never exercise the thermal model. The final
frontier therefore admits only points that exceed `100 °C` somewhere while
remaining below the pathology guards.

Those `100/180 °C` and 50% guards are experiment classifications, not asserted
F1 calibration targets. The two non-dominated parameter sets are inputs to
further study, not recommended coefficients.

## Databricks hand-off

The next campaign should:

1. replay the 15 engaged healthy sets independently;
2. densify neighborhoods around the healthy/hot transition;
3. stratify results by vehicle, era, circuit partition, and cooling level;
4. reserve new vehicle/circuit/seed combinations for final validation;
5. test whether one thermal family can represent every era or whether engine
   resources require era-specific authored coefficients;
6. preserve Rust as the sole evaluator of physical equations.

Only after physical targets and the per-era interpretation are reviewed should
the project consider a new experiment profile or catalog resource.

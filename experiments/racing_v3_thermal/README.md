# Racing Model V3 adaptive thermal screen

This governed local campaign is the second delivery slice of issue #287. It
uses the offline V8 profile and candidate `pitgun.racing-v3-candidate@0.10.0`;
it changes neither the game nor a published Racing Catalog.

The word **adaptive** describes a two-stage experiment:

1. a broad one-factor screen measures which thermal controls change pace,
   temperature, heat, derating or drag across short and long runs;
2. a deterministic Halton sequence varies only the observable controls
   together and concentrates compute on their interactions. Derating controls
   join a declared dependency closure when an upstream heat or threshold axis
   is active, even if the neutral baseline does not cross the thermal limit;
3. a local refinement samples both sides of the observed transition between a
   thermally disengaged engine and pathological overheating.

The campaign covers six enabled-era anchors, six circuit archetypes split into
calibration and held-out groups, and three cooling settings. Neutral cooling is
replayed with a second seed. The default 64-sample campaign therefore executes
more than fifteen thousand deterministic simulations; `--adaptive-samples 128`
doubles the broad joint surface without changing the protocol.

Build the immutable Rust probe and run the default campaign:

```bash
cargo build --release -p pitgun-racing-simulator \
  --example v3_decision_surface_probe
python3 experiments/racing_v3_thermal/screen_local.py \
  --jobs 12 \
  --adaptive-samples 64 \
  --refinement-samples 32
```

Replay the stored evidence byte for byte with the same runner:

```bash
python3 experiments/racing_v3_thermal/screen_local.py \
  --jobs 12 \
  --adaptive-samples 64 \
  --refinement-samples 32 \
  --check
```

The report includes raw deterministic points, activation effects, aggregated
cooling responses, pathological regions and a three-objective Pareto frontier.
That frontier is diagnostic only. A parameter set cannot be promoted until
physical targets are declared and the retained regions pass the independent
Databricks replay.

## Current V1 result

The canonical 64 + 32 campaign completed 15,192 executions and 263,196 laps.
It found 15 thermally engaged healthy parameter sets, 36 disengaged sets and
45 sets with at least one pathological execution. Five axes are independently
active, three derating axes are retained conditionally, and the static-cooling
multiplier is structurally inactive because its catalog reference is zero.

The interpretation and exact Databricks hand-off are documented in
[`RACING_V3_THERMAL_LOCAL_SCREEN_V1.md`](../../docs/RACING_V3_THERMAL_LOCAL_SCREEN_V1.md).

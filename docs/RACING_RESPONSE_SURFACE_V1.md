# Racing Response Surface V1

## Decision

Do not publish a circuit-aware opponent policy from the first Databricks sweep
and do not change model coefficients yet. A deterministic local grid confirms
that the existing downforce response is saturated at its upper bound on every
reviewed circuit, while gearing has a real but much weaker effect.

The next experiment should isolate candidate aerodynamic, gearing, and circuit
classification coefficients behind an offline calibration boundary. Only a
reviewed result may become a versioned Racing Catalog resource.

## Question

Why did `high-downforce-short-gearing` win the governed 105-run Databricks
campaign on all five representative circuits?

PR #172 first added `pitgun.racing-setup-response/v1` diagnostics to the
canonical Solver. The local V1 response surface then evaluated eleven equally
spaced downforce levels and eleven gearing levels on five physical circuits at
seed `42`: 605 canonical one-lap simulations.

The experiment was executed twice. The second complete execution passed
`--check`, proving byte-identical JSON and checksum output for the same release
runner and inputs.

| Evidence | Identity |
|---|---|
| Report schema | `pitgun.racing-response-surface/v1` |
| Runner | `pitgun 0.1.0-alpha.1` |
| Runner digest | `sha256:2335e6ea5391be9f28dfdb29eabe7c76c8c5691a2c41a86c888caa249614b1c4` |
| Report digest | `sha256:af5d0b7ec25387b236af14fd20ca4d1893d86dc726218be5c066db82e0139288` |
| Successful points | 605 / 605 |

## Findings

### Circuit data is differentiated, but incomplete

The five circuit resources are not aliases. Their lengths range from 3,324 m
to 5,811 m, absolute curvature integrals from 25.13 rad to 32.92 rad, and
classified corner-distance shares from 61.7% at Monza to 90.9% at Monaco.

However, every reviewed circuit has zero elevation gain and loss. Monza's 61.7%
classified corner share is also high for the intended power-circuit archetype.
The physical resources and the `0.001 rad/m` straight/corner boundary therefore
remain calibration hypotheses rather than editorial truth.

### Downforce dominates the lap-time response

At midpoint gearing, maximum versus minimum downforce produced:

| Circuit | Lap-time delta | Maximum-speed delta | Corner-speed delta | Added drag work |
|---|---:|---:|---:|---:|
| Monza | -4,884 ms | -16.44 km/h | +18.00 km/h | +7,276 kJ |
| Monaco | -4,579 ms | -14.86 km/h | +12.76 km/h | +3,228 kJ |
| Budapest | -7,100 ms | -6.18 km/h | +19.92 km/h | +4,701 kJ |
| Suzuka | -7,011 ms | -20.60 km/h | +22.36 km/h | +7,768 kJ |
| Singapore | -5,474 ms | -18.26 km/h | +14.21 km/h | +4,787 kJ |

Negative lap-time deltas mean maximum downforce is faster. The expected
trade-off exists: top speed falls and drag work rises. It is simply unbalanced;
the corner-speed gain overwhelms the cost on every circuit.

All 55 fixed-gearing slices improved strictly at every downforce step from
`0.0` through `1.0`. The fastest point therefore used downforce `1.0` on all
five circuits. This is a saturated control, not a useful setup decision.

The current tuning transformation helps explain the shape. Across the slider,
the downforce blend rises from `0.75` to `1.30` (+73% relative), while the drag
blend rises from `0.85` to `1.15` (+35% relative). These values are hypotheses
to test, not yet values to replace.

### Gearing works, but is weakly discriminating

At midpoint downforce, longest versus shortest gearing raised maximum speed by
0.7 to 5.0 km/h, proving that the control reaches the physical model. It still
lost only 158 to 256 ms per lap. Four circuits selected the shortest tested
gearing (`0.0`); Monza selected `0.2`.

Observed maximum RPM utilization remained between 79.0% and 87.5% across the
complete grid, and no point spent time above the diagnostic's 98% near-limit
threshold. That observation alone does not prove an engine defect: the power
curve may correctly peak below the 15,000 RPM hard limit. It does show that
rev-limit pressure currently contributes no setup trade-off.

## Interpretation

The universal optimum is not caused by an inert simulation or identical
circuits. It is explained by three measured facts:

1. aerodynamic setup authority is much larger than gearing authority;
2. the downforce benefit remains monotonic through the maximum allowed value;
3. the selected circuit resources contain many classified corner metres and no
   elevation, limiting other sources of differentiation.

These findings reject two tempting shortcuts: inventing circuit-specific AI
setups unsupported by evidence, or adding hidden gameplay bonuses after the
simulation.

## Next experiment

Build an offline, versioned coefficient study around the existing equations:

1. vary downforce-slider span and drag-slider span independently;
2. vary gear-ratio span while retaining the same engine power curve;
3. test the curvature classification boundary against the physical circuit
   samples;
4. retain monotonic physical invariants and compare at least Monza, Monaco, and
   Suzuka first;
5. propose catalog coefficients only if at least two circuit classes develop
   materially different interior optima;
6. review the resulting campaign manifest before spending Databricks compute.

The experiment must not add circuit labels to the Solver, tune against player
data, or make Databricks part of online gameplay.

## Reproduction

```bash
cargo build --release -p pitgun-cli
python3 experiments/racing_response/response_surface.py --jobs 8
python3 experiments/racing_response/response_surface.py --jobs 8 --check
```

The complete normalized evidence and checksum live in
`experiments/racing_response/results/`.

# Racing aerodynamic base calibration V1

## Decision

The local base calibration proposes the following complete aerodynamic
response as the candidate for a governed Databricks replay:

- `drag_base = 0.650`;
- `drag_slider_gain = 0.950`;
- `downforce_base = 1.05625`;
- `downforce_slider_gain = 0.375`;
- every other `TuningResponseV1` coefficient left at its historical default.

This response is not published to the game or catalog. It is an offline
candidate that restores historical gameplay pace while retaining the reviewed
circuit-dependent setup shape.

## Why the bases had to move

The shape experiment changed the slider gains but kept the historical bases.
That unintentionally moved the aerodynamic response at the neutral slider.

At slider `0.5`, the historical blends are:

- drag: `0.85 + 0.30 × 0.5 = 1.00`;
- downforce: `0.75 + 0.55 × 0.5 = 1.025`.

With the selected gains, exact neutral preservation implies bases of `0.525`
for drag and `0.8375` for downforce. That interpretable point reduced the
root-mean-square pace gap from 4,745.766 ms to 2,181.269 ms, confirming the
hypothesis, but it did not fully restore the historical envelope.

## Protocol

The coarse calibration crosses five drag bases and five downforce bases around
the neutral-preservation point. It includes the shape anchor and historical
response, for 9,801 simulations over:

- Monza, Monaco, and Suzuka;
- eleven downforce values;
- eleven gearing values;
- seed `42` and one lap per point.

Because the coarse winner reached the upper downforce boundary, a second
five-by-five interior refinement repeats another 9,801 simulations around the
extended optimum. The refined report records the digest of the coarse report.

Candidates must:

- preserve every aerodynamic invariant;
- retain materially separated `Monza < Suzuka < Monaco` downforce optima;
- keep at least 5 km/h below the campaign's 400 km/h speed guardrail;
- improve the shape anchor's historical-pace gap;
- sit inside both axes of the refined parameter space.

## Selected candidate

| Circuit | Downforce optimum | Gearing optimum | Candidate lap | Gap to historical optimum |
| --- | ---: | ---: | ---: | ---: |
| Monza | `0.0` | `0.4` | 82,717 ms | −293 ms |
| Monaco | `1.0` | `0.0` | 63,149 ms | −199 ms |
| Suzuka | `0.3` | `0.0` | 88,413 ms | +527 ms |

The root-mean-square compatibility gap is 366.597 ms, down from 4,745.766 ms
for the uncalibrated shape anchor. The maximum observed speed is 384.449 km/h,
leaving 15.551 km/h below the numerical cap.

At midpoint gearing, maximum rather than minimum downforce:

- costs 2,297 ms at Monza;
- gains 1,504 ms at Monaco;
- costs 527 ms at Suzuka, whose optimum lies at `0.3`.

Maximum speed falls by roughly 48–63 km/h while mean corner speed increases on
all three circuits. The desired trade-off remains explicit.

## Limitations

The Suzuka optimum leads its adjacent downforce value by only 6 ms. It should
be interpreted as a broad mixed-circuit plateau, not a physically precise
setting. The 0.3 slider-unit separation from each extreme is nevertheless
material at the policy-family level.

Historical pace is only a gameplay-compatibility target. The selected bases do
not prove real-world aerodynamic accuracy. Operational telemetry, more circuit
models, multi-lap behavior, and multiple seeds remain outside this local pass.

## Next governed step

The next campaign should freeze the four proposed coefficients in an immutable
experimental manifest and replay a deliberately small setup neighborhood:

- all five representative physical circuits;
- the three governed seeds already used by Pitgun;
- the candidate and historical responses;
- local optimum neighborhoods rather than another unconstrained search.

Databricks should persist the execution lineage in Delta and the experiment in
MLflow. It must only produce a reviewed proposal; publishing a model or catalog
release remains a repository decision.

## Reproduction

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/calibrate_aero_bases.py --jobs 4
python3 experiments/racing_response/refine_aero_bases.py --jobs 4
python3 experiments/racing_response/calibrate_aero_bases.py --jobs 4 --check
python3 experiments/racing_response/refine_aero_bases.py --jobs 4 --check
```

The two compact reports and their checksums live under
`experiments/racing_response/results/`.

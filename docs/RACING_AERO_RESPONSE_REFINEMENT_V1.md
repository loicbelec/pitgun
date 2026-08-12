# Racing aerodynamic response refinement V1

## Decision

The refined local campaign selects the following response-shape anchor for the
next absolute-pace calibration experiment:

- `downforce_slider_gain = 0.375`;
- `drag_slider_gain = 0.950`;
- every other `TuningResponseV1` coefficient left at its historical default.

This is not a production coefficient proposal. It is the eligible response
shape with the smallest root-mean-square pace disruption relative to the
historical model on the reviewed grid.

## Protocol

The deterministic campaign evaluates:

- 42 refined response families plus the historical reference;
- Monza, Monaco, and Suzuka;
- eleven downforce and eleven gearing values from `0.0` to `1.0`;
- seed `42` and one lap per point;
- 15,609 simulations in total.

Only the downforce and drag slider gains vary. Compact time and maximum-speed
matrices preserve the reviewed surface, while a digest identifies all complete
execution points without committing a much larger verbose point list.

A shape is eligible only when:

- all four aerodynamic invariants pass at every gearing level;
- downforce optima follow `Monza < Suzuka < Monaco`;
- Suzuka has an interior optimum;
- adjacent circuit optima differ by at least `0.2` slider units.

Twenty-four of the 42 refined families pass these rules.

## Selected response shape

| Circuit | Downforce optimum | Gearing optimum | Lap time | Gap to historical optimum |
| --- | ---: | ---: | ---: | ---: |
| Monza | `0.0` | `0.2` | 87,921 ms | +4,911 ms |
| Monaco | `1.0` | `0.0` | 66,263 ms | +2,915 ms |
| Suzuka | `0.6` | `0.0` | 93,798 ms | +5,912 ms |

The root-mean-square compatibility gap is 4,745.766 ms. This metric compares
gameplay pace with the historical model; it is not evidence of real-world
physical accuracy.

At midpoint gearing, maximum versus minimum downforce:

- costs 1,341 ms at Monza;
- gains 1,667 ms at Monaco;
- gains 944 ms at Suzuka.

Maximum speed falls by about 43–58 km/h while corner speed increases on all
three circuits. The response therefore creates a real straight-line versus
cornering trade-off rather than an editorial circuit bonus.

## Interpreting the Suzuka plateau

The selected Suzuka optimum at `0.6` leads the next downforce value by only
3 ms. Monza and Monaco have adjacent-setting margins of 56 ms and 126 ms. The
exact Suzuka number should therefore be understood as the center of a broad
mixed-circuit plateau, not as a physically precise optimum.

This is not caused by driver randomness. For a fixed seed, driver, and lap, the
Solver draws the same additive lap noise for every setup configuration. The
noise cancels when configurations are ranked. A multi-seed replay remains
valuable for the governed Databricks campaign, but it cannot turn this plateau
into a sharper physical distinction.

## Why nothing is published yet

The refined shape still slows the representative laps by several seconds.
Publishing it now would change game balance, opponent strength, verification
identities, and leaderboard comparability.

The next local experiment should hold the selected slider gains fixed and vary
only `drag_base` and `downforce_base`. Its objectives are separate and ordered:

1. retain the reviewed circuit-dependent response shape and invariants;
2. reduce the compatibility pace gap against the historical model;
3. report any remaining disagreement rather than hiding it in a gameplay
   bonus.

That search will preserve historical pace as a product-compatibility target,
not as a claim of physical truth. Calibration against operational telemetry is
a later, separately governed step.

Only after the base-coefficient experiment should Pitgun freeze a candidate
manifest and replay it through Databricks across five circuits and multiple
seeds.

## Reproduction

From the repository root:

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/refine_aero_response.py --jobs 4
python3 experiments/racing_response/refine_aero_response.py --jobs 4 --check
```

The compact evidence and checksum live in
`experiments/racing_response/results/racing-aero-response-refinement-v1.json`
and its adjacent `.sha256` file.

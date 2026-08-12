# Racing coefficient screening V1

## Decision

The first bounded screening found three aerodynamic response families worth a
deeper calibration pass. It did **not** select or publish new physical
coefficients.

The strongest first candidate is:

- `downforce_slider_gain = 0.25`;
- `drag_slider_gain = 0.75`;
- every other `TuningResponseV1` coefficient left at its historical default.

On the five-level setup grid this family produces a low-downforce optimum at
Monza (`0.0`), a high-downforce optimum at Monaco (`1.0`), and an interior
compromise at Suzuka (`0.75`). Its four declared aerodynamic invariants pass on
all three circuits.

This is evidence about the **shape** of the response, not evidence that the
absolute physics or lap times are calibrated.

## Protocol

The local, deterministic campaign evaluates:

- 25 coefficient families: five downforce gains crossed with five drag gains;
- three representative physical circuits: Monza, Monaco, and Suzuka;
- five downforce levels and five gearing levels from `0.0` to `1.0`;
- seed `42` and one lap per point;
- 1,875 simulations in total.

Only `downforce_slider_gain` and `drag_slider_gain` vary. Changing more physical
families at once would make the cause of an improved response ambiguous.

A candidate remains eligible only when:

- increasing downforce reduces maximum speed;
- increasing downforce raises corner speed, drag work, and mean downforce;
- at least one circuit has an interior downforce optimum;
- all three circuits have distinct downforce optima;
- the optimum range spans at least one grid interval (`0.25`).

## Results

Three of the 25 families pass those rules:

| Candidate | Monza optimum | Monaco optimum | Suzuka optimum |
| --- | ---: | ---: | ---: |
| downforce `0.25`, drag `0.75` | `0.0` | `1.0` | `0.75` |
| downforce `0.25`, drag `0.90` | `0.0` | `1.0` | `0.25` |
| downforce `0.35`, drag `0.90` | `0.0` | `1.0` | `0.75` |

For the leading family, changing from minimum to maximum downforce at midpoint
gearing costs 1,446 ms at Monza, gains 1,009 ms at Monaco, and gains 429 ms at
Suzuka. Maximum speed falls by roughly 34–46 km/h, while mean corner speed rises
by roughly 2.3–4.7 km/h. The trade-off is therefore visible and physically
directed rather than an arbitrary scoring penalty.

The historical family (`downforce 0.55`, `drag 0.30`) ranks 20th: every circuit
still chooses maximum downforce, so it fails the differentiation rule despite
passing the basic physical invariants.

## Important limitation

All shortlisted families make their fastest laps several seconds slower than
the historical family. That is expected because this pass changed response
shape without rebalancing the absolute aerodynamic baselines. Publishing one
now would alter game pace, verification identities, opponent balance, and
leaderboard comparability.

The next experiment should refine the three shortlisted neighborhoods and
separate two goals:

1. preserve meaningful circuit-dependent setup trade-offs;
2. restore a governed absolute pace envelope using reference telemetry or
   defensible engineering targets.

Only after that should a versioned catalog candidate be proposed and replayed
through Databricks at larger scale.

## Reproduction

From the repository root:

```bash
cargo build --release -p pitgun-racing-simulator --example tuning_response_probe
python3 experiments/racing_response/coefficient_screening.py --jobs 4
python3 experiments/racing_response/coefficient_screening.py --jobs 4 --check
```

The full points, summaries, probe identity, and fixed inputs live in
`experiments/racing_response/results/racing-coefficient-screening-v1.json`; its
adjacent checksum identifies the exact report.

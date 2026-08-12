# Racing mixed-circuit diagnosis V1

## Decision

Keep the current calibrated aerodynamic response for mixed-circuit behavior.
Do not increase downforce response merely to make the coarse setup named
`balanced` win at Suzuka.

The Databricks review returned `REFINE` because its seven-point setup campaign
compared `low-downforce` at slider `0.2` with `balanced` at `0.5`. The finer
eleven-level surface places the deterministic Suzuka optimum at `0.3`: the
configuration labelled low-downforce wins because it is 0.1 from that optimum,
while the balanced configuration is 0.2 away. This is review-grid aliasing,
not evidence that the physical response prefers a low-downforce boundary.

## Protocol

The bounded local diagnosis evaluates 35 immutable response variants:

- `downforce_slider_gain`: 0.375 to 0.450 in increments of 0.0125;
- `corner_aero_scale`: 1.0500 to 1.1000 in increments of 0.0125;
- fixed calibrated drag base/gain and downforce base;
- eleven downforce by eleven gearing levels;
- seed 42;
- five selection circuits plus Silverstone and Spa as post-selection holdouts;
- 29,645 deterministic Rust probe executions.

The current response is the minimum-distance calibration-eligible candidate.
Its continuous downforce optima are:

| Circuit | Archetype | Optimum |
|---|---|---:|
| Monza | power | 0.0 |
| Monaco | high downforce | 1.0 |
| Budapest | mechanical grip | 1.0 |
| Suzuka | mixed | 0.3 |
| Singapore | street thermal | 1.0 |

The independent Silverstone surface places its optimum at 0.7. This supports
the interpretation that mixed circuits do not share one universal setup and
that the current response can produce an interior, circuit-dependent optimum.

## Holdout finding

Spa was not used to choose or rank a response. After selection, it exposed a
separate global guardrail failure:

- maximum observed speed: 406.393 km/h;
- fastest point: downforce 0.0, gearing 0.9, 99,992 ms;
- increasing downforce did not increase the diagnostic mean corner speed at
  any of the eleven gearing levels.

None of the downforce-gain/corner-scale variants fixes this, because the
experiment intentionally held drag coefficients constant. The finding must
therefore open a distinct high-speed/track diagnostic investigation rather
than contaminate the Suzuka response decision.

## Consequences

1. Replace named-family physical coherence with continuous optimum bounds in a
   future governed review campaign.
2. Include a scenario near downforce slider 0.3 for mixed-circuit evidence.
3. Keep the current candidate coefficients unchanged for this decision.
4. Investigate Spa's speed and corner-response behavior separately before any
   canonical promotion.
5. Preserve human approval: this report changes neither the catalog nor game.

The exact compact evidence is
[`racing-mixed-circuit-diagnosis-v1.json`](../experiments/racing_response/results/racing-mixed-circuit-diagnosis-v1.json)
with its adjacent SHA-256 checksum.

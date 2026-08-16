# Economy-backed Racing development-budget effect V2

This campaign replaces the synthetic progression totals used by V1 with the
public, reproducible Pitgun game-economy snapshots frozen in
`loicbelec/pitgun-game#159`.

| Progression | Reference | Below | Above |
| --- | ---: | ---: | ---: |
| early | 4 | 3 | 5 |
| mid | 27 | 24 | 30 |
| late | 37 | 33 | 41 |

Each of the 45 exact triplets keeps its circuit, seed, neutral player setup,
balanced one-stop strategy, and nine-opponent field fixed. The opponent field
is normalized to the matching reference total while retaining each authored
relative four-axis allocation as far as integer largest-remainder scaling
allows.
Only the player's total budget and balanced point allocation differ inside a
triplet.

The early treatment uses adjacent integers because rounding 90%, 100%, and
110% of four points would collapse all three treatments to the same value. At
the four-point reference itself, integer quantization also maps the audited
opponent allocations to one point per axis. Setup and strategy differences
remain present; the loss of development-allocation diversity is recorded as an
explicit early-progression caveat rather than hidden or manually corrected.

Regenerate the immutable scenarios and checksummed manifest with:

```bash
python3 experiments/budget_effect_v2/build_campaign.py
```

These assets contain no observed player, career, leaderboard, or telemetry
data. They select no difficulty policy and cannot promote a game or catalog
change automatically. V1 remains immutable historical evidence.

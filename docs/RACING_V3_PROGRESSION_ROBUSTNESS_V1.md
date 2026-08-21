# Racing Model V3 progression robustness V1

Status: governed local diagnostic for issue #283. No production, catalog,
opponent-policy or model-identity change is authorized by this document.

## Question

Does Model V3 `0.9.0` provide useful and transferable aero, chassis, cooling,
engine, setup and strategy decisions across the active vehicles and the actual
game-progression budgets?

## Controlled evidence

The campaign executes 4,928 deterministic three-lap cases (14,784 simulated
laps) over:

- active vehicles `classic_v8_1960`, `classic_v8_1970`, `modern_v6t` and
  `f1_2026`;
- economy-backed early, middle and late budgets of 4, 27 and 37 points;
- calibration circuits Monza, Monaco and Suzuka;
- held-out circuits Spa, Barcelona, Singapore and Mexico;
- seeds 7 and 42.

For each vehicle/circuit/progression anchor, direct +1/−1 measurements establish
absolute marginal activation. Every directed transfer between two axes then
measures specialization at constant total budget. A middle-budget 5 × 5
downforce/gearing grid measures setup specialization separately. The immutable
long-run report from issue #280 supplies compound and stop-window evidence by
digest.

All cases bind `pitgun.racing-v3-candidate@0.9.0` with digest
`sha256:d8767c911912c1ae19cf50f8bb2c6455f7308d83a762d6192f4ef090ef199d99`.
The canonical campaign artifact is
`experiments/racing_v3_progression_robustness/results/local-progression-robustness-v1.json`.

## Decision

| Capability | Verdict | Observation |
| --- | --- | --- |
| Multi-era development surface | `STRUCTURAL_CHANGE_REQUIRED` | cooling is inactive on historical V8s, threshold-like on modern vehicles, and chassis dominates middle/late `f1_2026` |
| Held-out setup specialization | `PASS` | 8 calibration optima and 10 held-out optima across the 5 × 5 grids |
| Generic strategy robustness | `REFINE` | generic L16 loses 8,572 ms median, but L22 is universally fastest |
| Production or opponent change | `REFINE` | diagnostic evidence only |

## Physical interpretation

Aero, chassis and engine have non-zero direct pace margins for every active
vehicle at all three progression anchors. Cooling behaves differently:

- both historical V8s remain below thermal derating in these short runs, so
  additional cooling capacity has no pace value;
- `modern_v6t` and `f1_2026` need cooling at the four-point boundary;
- by 27 points, the balanced allocation already supplies enough cooling to
  avoid derating, so another cooling point changes neither pace nor the audited
  thermal outcome.

This is not a reason to remove cooling. It is evidence that the current control
represents a binary derating threshold rather than a progressively valuable
thermal-management decision. The next candidate should explore a smoother
relationship among cooling capacity, radiator/drag cost, thermal state,
reliability or energy deployment. Those mechanisms must remain era-aware: a
1960s V8 should not inherit an unexplained modern thermal-management game.

At fixed budget, historical and middle/late modern allocations usually improve
by moving cooling points toward chassis or engine. `f1_2026` reverses sharply:
cooling is the universal early winner, then chassis is the universal middle and
late winner. Scalar retuning alone may relocate these crossovers, but cannot
make the threshold progressive. The Databricks campaign must therefore screen
both bounded coefficient changes and at least one explicit thermal-cost shape.

## Setup and strategy boundary

Downforce and gearing are already meaningfully circuit- and vehicle-dependent
for the three aerodynamic resources. The no-downforce `classic_v8_1960` has one
repeated coarse optimum: its downforce position cannot invent aerodynamic load,
while the shortest reviewed gearing wins the sampled tracks. That is an honest
vehicle capability boundary, not a missing catalog lookup.

Strategy remains a separate limitation. Compound wear is active and a generic
lap-16 stop carries measurable regret, yet the latest reviewed lap-22 stop wins
all 16 long-run groups. Opponent strategy generation should not be calibrated
until this universal boundary is challenged with broader race distances and
pit-loss/compound rules.

## Next gate

Issue #284 should replay the governed decision surface on Databricks using this
artifact as its local baseline. It must preserve calibration/held-out labels,
retain per-vehicle and per-progression verdicts, and reject automatic promotion.
Only after that governed replay should a new immutable V3 candidate alter aero,
chassis, cooling or engine parameters.

The replay contract is materialized as the checksummed
`experiments/databricks/campaigns/racing-v3-decision-surface-v1.json` manifest.
It deduplicates the physical inputs but retains all 4,928 natural run keys and
the expected digest of every full Rust result and compact evidence point. The
read-only response-surface notebook consumes the resulting Delta rows for
visual review; it is deliberately not a parameter registry or a promotion
engine.

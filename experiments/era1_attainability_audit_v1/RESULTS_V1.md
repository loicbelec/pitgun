# Era-I attainable player audit — development review PASS

Source/protocol freeze `77e2f0a`. Unchanged physical policy, native executable, reference driver and nine-rival field. This is an opened-seed development audit, not a new holdout or runtime admission.

| Item | Result |
|---|---|
| Fresh full GP / accepted laps | 153 / 9,027 |
| Logical preparation evaluations / unique primary solves | 162 / 147 |
| Exact id-only duplicate evaluations avoided | 15 |
| Reused rival GP / native input bridges | 81 / 81 |
| Numerical failures | 0 |
| Exact fresh full-result repeats | 6 / 6 |
| Exit free-preparation wins / median rank | 2 of 3 / P1 |
| Zero-extra family wins over all nine contexts | 0 |
| Development criteria | PASS |
| New field/runtime/catalog admission | Pending; V1 failed reserves retained |

## Best sampled results against the unchanged field

| Circuit | Effective cap | No extra aero/cooling | Heavy chassis/engine | Free preparation |
|---|---:|---:|---:|---:|
| MONZA | 8 | P10 | P9 | P1 |
| MONZA | 10 | P10 | P1 | P1 |
| MONZA | 12 | P10 | P8 | P1 |
| SILVERSTONE | 8 | P10 | P9 | P1 |
| SILVERSTONE | 10 | P10 | P1 | P1 |
| SILVERSTONE | 12 | P10 | P5 | P1 |
| ZANDVOORT | 8 | P10 | P6 | P3 |
| ZANDVOORT | 10 | P10 | P2 | P1 |
| ZANDVOORT | 12 | P10 | P5 | P3 |

Caps8and12 use actual career entry/exit fixtures, including exit chassis1. Cap10 is a controlled equal-equipment intermediate case, not a new authored career state. Heavy80is measured on discretionary points after HQ floors; that family allows cooling and may legitimately win. The search families overlap, and aliases are not independent samples.

## Actual exit means: constructive winning latitude

| Circuit | Effective A/C/E/K | Gear | Stop after lap | Free-player rank | Gap to best AI |
|---|---|---:|---:|---:|---:|
| MONZA | [0, 5, 4, 3] | 0.25 | 26 | P1 | -1.459s |
| SILVERSTONE | [0, 5, 4, 3] | 0.15 | 26 | P1 | -3.106s |
| ZANDVOORT | [0, 5, 4, 3] | 0.25 | 33 | P3 | +6.005s |

The free sample wins at Monza and Silverstone and reachesP3at Zandvoort without weakening traits or changing physical laws. It uses different gearing and pit timing by circuit. The bounded zero-extra-cooling/aero sample isP10in every context, despite its own gearing/timing/split adjustments. This supports meaningful preparation and cooling tradeoffs in the studied era-I contexts, not a prohibition on every engine/chassis-heavy setup.

## Limits and next gate

All observations use opened seed73013and a small declared candidate pool. This does not establish a human win rate, all-setup impossibility, population-level difficulty, independent aero sensitivity or real traffic/incident behavior. The old V1 reserve failures remain failed; this audit does not retroactively qualify them. New reserved budgets/seeds must follow a complete policy freeze.

The fixed field can be beaten more comfortably at the controlled intermediate cap10than at exit means; budget-dependent preparation remains worth reviewing in the complete field fit. The audit supplies reachable player preparations, not a certificate that every available budget has identical difficulty. Do not force every axis to receive points or force more cooling to win universally.

Next: freeze the era-II same-opportunity joint preparation matrix, measure the actual named drivers, retain imperfect but competitive policies, then new reserved evaluation, native/WASM/recovery/performance and a consolidated local I/II playtest. III, IV and V follow as separate complete era campaigns under #492. Current local preview and deployed services remain unchanged.

## Execution policy

The owner updated the preference from20-GP microbatches to completing one whole era at a time. This campaign finished in operational segments20+20+60+53; the earlier40GPfiles/mtimes and journal prefix remain unchanged. Smaller limits remain available for recovery; do not make the owner approve every microbatch. Keep a single native worker and no concurrent-era campaigns. Source and physical matrix stayed frozen while only scheduling arguments changed.

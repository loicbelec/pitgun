# Era-II qualification — actual career technology and bonus floors

Tracking #483 / #479 / draft#481. Same frozen three-compound policy as era I. No tire, engine, driver or cooling coefficients fitted here.

Numerical gate: **PASS**. 195 full-distance cases and 6 selected repeats; 0 physical/check failures, 0 non-exact repeats.

## Actual means

The two season programmes each have six rounds; their union contains seven regular circuits. The matrix covers that union, not invitations/Monaco. Allocation arrays below are discretionary; bonuses are then added once in Rust.

| Stage | Discretionary | A/C/E/K bonuses | Effective budget | Engine | Aero / downforce |
|---|---:|---|---:|---|---|
| entry | 11 | [0, 1, 0, 0] | 12 | v8_1960 | none / locked .5 |
| exit | 16 | [2, 1, 4, 1] | 24 | v8_1970 | basic / adjustable |

Entry/exit timing differences combine earned budget and installed technology. They do not identify an isolated engine-architecture effect. Zero discretionary cooling leaves Cooling1 at exit. Same driver reference traits, initial110kg and paired primary seed; reserved seed and modes are explicit.

## Best sampled normal preparation at default sliders

Three point profiles xthree normal recipes per stage/circuit. These are bounded candidates, not global optima or opponent-win probabilities. All totals include catalog service loss.

| Circuit | Entry preparation / plan | Entry total s | Exit preparation / plan | Exit total s |
|---|---|---:|---|---:|
| MONTREAL | cooling / SM-50 | 8020.369 | cooling / MH-50 | 7004.325 |
| SPIELBERG | cooling / SM-50 | 7731.504 | cooling / MH-50 | 6746.842 |
| MEXICO | cooling / SM-50 | 8627.411 | no_discretionary_cooling / MH-50 | 7624.644 |
| SAOPAULO | cooling / SM-50 | 7847.498 | no_discretionary_cooling / MH-50 | 6988.425 |
| SILVERSTONE | cooling / SM-50 | 7669.185 | no_discretionary_cooling / MH-50 | 6792.924 |
| MONZA | cooling / SM-50 | 7310.583 | no_discretionary_cooling / MH-50 | 6524.487 |
| ZANDVOORT | cooling / SM-50 | 8253.678 | no_discretionary_cooling / MH-50 | 7295.037 |

## Discretionary cooling tradeoff at the same means

Best of the same three normal plans for each profile, same .5/.5 sliders. Negative deltas are legitimate when inherited cooling/engine design makes additional discretionary cooling unhelpful in this sample.

| Circuit | Stage | Balanced total s | No discretionary Cooling total s | Cooling-oriented total s | No discretionary Cooling minus best other s |
|---|---|---:|---:|---:|---:|
| MONTREAL | entry | 8083.750 | 8248.229 | 8020.369 | +227.860 |
| MONTREAL | exit | 7015.023 | 7013.952 | 7004.325 | +9.628 |
| SPIELBERG | entry | 7798.614 | 7963.279 | 7731.504 | +231.775 |
| SPIELBERG | exit | 6756.099 | 6750.741 | 6746.842 | +3.899 |
| MEXICO | entry | 8680.600 | 8817.348 | 8627.411 | +189.938 |
| MEXICO | exit | 7636.379 | 7624.644 | 7626.696 | -2.052 |
| SAOPAULO | entry | 7901.494 | 8041.705 | 7847.498 | +194.208 |
| SAOPAULO | exit | 7004.997 | 6988.425 | 6992.894 | -4.469 |
| SILVERSTONE | entry | 7723.979 | 7838.796 | 7669.185 | +169.611 |
| SILVERSTONE | exit | 6811.143 | 6792.924 | 6800.874 | -7.950 |
| MONZA | entry | 7372.822 | 7532.239 | 7310.583 | +221.656 |
| MONZA | exit | 6543.446 | 6524.487 | 6532.640 | -8.153 |
| ZANDVOORT | entry | 8302.975 | 8412.995 | 8253.678 | +159.317 |
| ZANDVOORT | exit | 7315.384 | 7295.037 | 7303.642 | -8.605 |

## Stop-count controls on the same exit/balanced preparation

Zero/three-stop cases are physical diagnostics and do not change game/leaderboard stint rules. One/two-stop cells use only the sampled normal recipes.

| Circuit | Best 0-stop total s | Best 1-stop total s | Sampled 2-stop total s | Sampled 3-stop total s | Three beats sampled one/two |
|---|---:|---:|---:|---:|---|
| MONTREAL | 7084.865 | 7015.023 | 7056.829 | 6996.843 | True |
| SPIELBERG | 6814.048 | 6756.099 | 6795.271 | 6745.220 | True |
| MEXICO | 7743.062 | 7636.379 | 7685.551 | 7623.801 | True |
| SAOPAULO | 7148.849 | 7004.997 | 7070.276 | 6952.569 | True |
| SILVERSTONE | 6919.331 | 6811.143 | 6864.663 | 6774.608 | True |
| MONZA | 6625.150 | 6543.446 | 6581.447 | 6507.035 | True |
| ZANDVOORT | 7468.608 | 7315.384 | 7387.050 | 7256.230 | True |

## One-factor controls on the developed car

Fixed exit/balanced SM midpoint plan. Each slider is varied separately; entry downforce stays locked.

| Circuit | Gear .35 minus .5 s | Gear .85 minus .5 s | Downforce .2 minus .5 s | Downforce .8 minus .5 s |
|---|---:|---:|---:|---:|
| MONTREAL | -4.963 | -1.330 | -69.897 | +83.625 |
| SPIELBERG | -9.972 | +0.140 | -74.584 | +99.015 |
| MEXICO | +0.545 | -8.505 | -48.079 | +61.274 |
| SAOPAULO | +0.316 | +7.046 | -67.767 | +74.489 |
| SILVERSTONE | +0.640 | +4.033 | -64.174 | +83.444 |
| MONZA | -2.513 | +15.407 | -82.333 | +104.279 |
| ZANDVOORT | -1.517 | +2.202 | -50.010 | +60.816 |

## Thermal response on the balanced SM plan

Native last-lap surface/carcass means and minimum retained engine capacity. Different circuit temperatures are expected; no uniform operating-band requirement. These aggregate thermal states are not identified experimental tire measurements.

| Circuit | Stage | Last surface °C | Last carcass °C | Minimum thermal power factor | Final tire wear % |
|---|---|---:|---:|---:|---:|
| MONTREAL | entry | 90.1 | 98.4 | 0.848 | 49.2 |
| MONTREAL | exit | 131.0 | 146.9 | 1.000 | 73.2 |
| SPIELBERG | entry | 90.9 | 99.5 | 0.844 | 47.9 |
| SPIELBERG | exit | 132.4 | 149.2 | 1.000 | 71.5 |
| MEXICO | entry | 90.2 | 97.8 | 0.850 | 57.0 |
| MEXICO | exit | 126.5 | 140.5 | 1.000 | 79.4 |
| SAOPAULO | entry | 93.1 | 101.6 | 0.847 | 54.0 |
| SAOPAULO | exit | 135.0 | 150.3 | 1.000 | 88.7 |
| SILVERSTONE | entry | 92.8 | 101.6 | 0.857 | 50.5 |
| SILVERSTONE | exit | 134.4 | 150.6 | 1.000 | 79.4 |
| MONZA | entry | 92.8 | 102.1 | 0.859 | 47.0 |
| MONZA | exit | 137.3 | 155.1 | 1.000 | 75.0 |
| ZANDVOORT | entry | 93.3 | 101.3 | 0.851 | 58.3 |
| ZANDVOORT | exit | 132.3 | 146.2 | 1.000 | 93.1 |

## Driver modes and reserved seed

Three contexts only for mode comparison, fixed exit/balanced SM. Recoverable lapses are not crashes/VSC; these samples do not calibrate the long-run risk/reward tradeoff.

| Circuit | Mode | Delta Balanced total s | Recoverable lapses |
|---|---|---:|---:|
| MONTREAL | manage | +15.553 | 0 |
| MONTREAL | attack | -13.932 | 2 |
| SILVERSTONE | manage | +15.603 | 0 |
| SILVERSTONE | attack | -5.606 | 2 |
| MONZA | manage | +12.696 | 0 |
| MONZA | attack | -5.693 | 2 |

Reserved seed104729 was fixed before execution; no fitting on it. A single extra seed does not establish broad stochastic robustness.

## Numerical evidence and decision

- All actual bonus floors, effective budgets and installed component selections are checked against retained career fixtures.
- Initial preparation, both tire temperatures, wear, speed and engine temperature carry are checked at every native boundary.
- Catalog service time is included in total elapsed and independently checked from per-lap driving times.
- 201 retained successful GP / 13158 driven laps; 195 distinct main physical inputs.
- Maximum carry jump 0; tire segment energy residual 6.69e-09J; elapsed residual 1.82e-12s.
- Selected fresh repeats: 6/6 exact physical-input and full-result digests.
- Sampled three-stop counterexamples: ['MONTREAL', 'SPIELBERG', 'MEXICO', 'SAOPAULO', 'SILVERSTONE', 'MONZA', 'ZANDVOORT']. This does not prove a global optimum or exclude other three-stop schedules.
- Native source `9d7b082e143f0ce878162ae858e3a8b37520baf0`; executable SHA256 `9acece92aef9cd61ffb5ba11345b58060f06a913f0e7775b637ba31806a0993e`.
- Operating-policy file SHA256 `0021bd8ead91698de49de297b1760cc3225a54c9e61adfeb07bab8250ab16439`. Inputs, per-lap scalars, receipts, failures and complete source/fixture/binary pins are retained.
- Numerical PASS supports proceeding to a separately versioned local browser candidate. It does not admit a leaderboard model or qualify nine-AI fairness, all budgets, native/WASM equality, real tire identification or industrial accuracy.
- Strategic gate remains open: all seven developed-car three-stop controls beat the sampled one/two-stop schedules. Diagnose this before freezing opponents; a local preview is exploratory only.
- Next: native-to-browser coupling and clear derating/braking display, then owner playtest before final budget-aware imperfect nine-AI preparation. No physical retune or deployment is implied.

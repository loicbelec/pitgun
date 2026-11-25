# Era-I strategy qualification — frozen operating candidate

Tracking: #482 / #479 / draft#481. No physical parameters changed during this campaign.

Numerical gate: **PASS**. 114 main cases, 18 gear-only controls and 12 applicable validation executions; 0 cases with failures/check violations, 0 failed equality checks.

## Scope

Single-car full-distance native runs, eight effective points, same baseline engine/car/110kg/reference traits. The five preparation samples include three allocation profiles and two paired slider contrasts. Modes and reserved seed are separate comparisons. No rivals or AI setups are fitted. Zero stops is a physical control, not a change to sporting rules. All rankings below include catalog service loss.

## Best sampled physical preparation

Balanced/primary-seed cases only, including diagnostic0/3 stops and paired slider samples. This is not a career-legal recommendation: era-I baseline downforce is locked at.5, and the game requires at least two stints. This is a bounded sample, not a global optimum.

| Circuit | Preparation / recipe | Stops | Total s | Driving s | Service s |
|---|---|---:|---:|---:|---:|
| MONZA | high_downforce_short / SM-50 | 1 | 7363.535 | 7341.535 | 22.0 |
| SILVERSTONE | cooling / SM-50 | 1 | 7728.547 | 7706.547 | 22.0 |
| ZANDVOORT | cooling / SM-50 | 1 | 8308.078 | 8286.078 | 22.0 |

## Stop-count comparison at the same balanced preparation

Only the nine preregistered core recipes on2222/.5/.5. Values are total GP seconds; each cell is the best sampled recipe for that stop count.

| Circuit | 0 stops | 1 stop | 2 stops | 3 stops | Sampled best |
|---|---:|---:|---:|---:|---|
| MONZA | 7406.520 | 7383.414 | 7413.731 | 7486.493 | SM-50 |
| SILVERSTONE | 7795.148 | 7734.768 | 7761.497 | 7841.856 | SM-50 |
| ZANDVOORT | 8458.392 | 8314.692 | 8333.062 | 8420.496 | SM-50 |

## Allocations under the same nine strategy samples

A/C/E/K: balanced2222, concentrated0440, cooling0332. Default .5/.5 sliders and equal effective budget. This comparison does not optimize the concentrated setup's sliders or all legal allocations.

| Circuit | 2222 best total s | 0440 best total s | 0332 best total s | 0440 minus best other s |
|---|---:|---:|---:|---:|
| MONZA | 7383.414 | 7545.017 | 7377.435 | +167.582 |
| SILVERSTONE | 7734.768 | 7852.845 | 7728.547 | +124.298 |
| ZANDVOORT | 8314.692 | 8430.182 | 8308.078 | +122.103 |

## Driver modes at fixed preparation and SM timing

Same reference traits and noise stream. Recoverable errors are native driver lapses, not crashes or VSC.

| Circuit | Mode | Total s | Delta Balanced s | Minimum power factor | Derated driving s | Recoverable errors |
|---|---|---:|---:|---:|---:|---:|
| MONZA | manage | 7441.479 | +58.066 | 0.861 | 7358.0 | 0 |
| MONZA | balanced | 7383.414 | +0.000 | 0.860 | 7300.7 | 1 |
| MONZA | attack | 7327.239 | -56.175 | 0.859 | 7245.0 | 2 |
| SILVERSTONE | manage | 7809.854 | +75.086 | 0.859 | 7729.5 | 0 |
| SILVERSTONE | balanced | 7734.768 | +0.000 | 0.858 | 7655.7 | 1 |
| SILVERSTONE | attack | 7672.321 | -62.448 | 0.857 | 7593.8 | 2 |
| ZANDVOORT | manage | 8410.040 | +95.348 | 0.854 | 8327.6 | 0 |
| ZANDVOORT | balanced | 8314.692 | +0.000 | 0.853 | 8233.0 | 1 |
| ZANDVOORT | attack | 8238.505 | -76.187 | 0.851 | 8157.3 | 2 |

Attack remains faster in all three primary-seed comparisons, despite more recoverable errors. This does not establish an economically balanced risk/reward tradeoff over many seeds; no accident/VSC claim is made.

## Timing and slider response

Each contrast keeps eight points. Slider variants change downforce and gearing together; differences do not isolate a single coefficient.

| Circuit | SM early minus midpoint s | SM late minus midpoint s | Low drag/long gear minus default SM s | High downforce/short gear minus default SM s |
|---|---:|---:|---:|---:|
| MONZA | +11.972 | +8.549 | +12.151 | -19.879 |
| SILVERSTONE | +12.033 | +18.656 | -4.919 | -0.190 |
| ZANDVOORT | +11.790 | +65.590 | +39.938 | -1.320 |

## Fresh-set response in the fixed balanced SM plan

First lap after service and subsequent mean surface temperatures. A low near-peak occupancy is a reported physical response, not an automatic failure. It combines load, tire preparation and model time constants; no display remapping or imposed target is applied.

| Circuit | First new-set lap | First-lap surface °C | First-lap peak-window time % | Next two surface means °C |
|---|---:|---:|---:|---|
| MONZA | 27 | 69.6 | 0.0 | 73.7, 77.3 |
| SILVERSTONE | 27 | 70.0 | 0.0 | 74.3, 77.9 |
| ZANDVOORT | 37 | 69.8 | 0.0 | 73.1, 76.1 |

## Reserved seed

No tuning on this seed. A one-seed repeatability test is distinct from stochastic robustness.

| Circuit | Reserved minus primary total s | Reserved recoverable errors |
|---|---:|---:|
| MONZA | +1.673 | 3 |
| SILVERSTONE | +2.375 | 3 |
| ZANDVOORT | +2.875 | 3 |

## Decision and limits

- Fresh full-result repeats (including corrected controls): 12/12 exact. This is repeatability, not equivalence to a different boundary algorithm.
- Three-stop counterexamples beating the sampled one/two-stop recipes: 0. None in this matrix; not a proof against other three-stop schedules.
- The sampled0440 setup dominates the two other core profiles on every circuit: False. This is a narrow setup test, not an exclusion of all concentrated configurations.
- Different circuit temperatures and compound responses are accepted. Do not retune simply to put every setup inside a common temperature band.
- No player win probability or nine-AI competitiveness claim: only one car is solved. No new game runtime, physics/catalog admission, provider call, verification submission or deployment.
- Native source commit: `c5972a6e404f4bf816db13dd000c6ba52e459792`; binary SHA256 `da3b9a20e7430826513f7287a62aaa2231a117a43aba30afc38b23753d2f6f42`.
- Frozen operating-policy file SHA256: `0021bd8ead91698de49de297b1760cc3225a54c9e61adfeb07bab8250ab16439`. Full pin, cases, receipts, per-lap scalars and failure records are retained in the compressed dataset.
- 132 named primary/control cases resolve to 126 distinct physical inputs. Six paired slider cases resolve identically to the legal gear-only controls: the baseline aero does neutralize the unavailable downforce setting.
- Each runner is serial. The tail of primary validations and the initial gear controls briefly overlapped, at most two native processes. Wall times are diagnostic, not a strict single-worker performance benchmark.

### Retained harness error and corrected validation

The frozen plan mistakenly selected 6 legacy invariant-work recomputation checks for ReachableWearV1, which explicitly rejects that option before solving. All six rejections remain in the primary dataset; they are not physical failures or equal-output comparisons. A separately frozen control repeats those same six physical cases with the supported mode. Together with the original six repeats,12full-result repeats are assessed above. No solver change or full-campaign rerun was used to hide the harness mistake.

## Legal gear-only control

Post-plan extension, frozen before its runs: original paired sliders confound gearing with downforce, and baseline era-I downforce is locked in the game.18additional SM runs keep downforce.5 and cover all three point profiles/circuits at gear.35/.85. No coefficients or primary cases changed. This checks the confound without choosing favorable preparations from observed times.

| Circuit | Allocation profile | Gear .35 minus .5 s | Gear .85 minus .5 s |
|---|---|---:|---:|
| MONZA | balanced | -19.879 | +12.151 |
| MONZA | engine_chassis | -24.886 | -9.792 |
| MONZA | cooling | -19.693 | +12.468 |
| SILVERSTONE | balanced | -0.190 | -4.919 |
| SILVERSTONE | engine_chassis | +16.246 | -19.583 |
| SILVERSTONE | cooling | +1.097 | -4.880 |
| ZANDVOORT | balanced | -1.320 | +39.938 |
| ZANDVOORT | engine_chassis | -3.376 | +47.500 |
| ZANDVOORT | cooling | -1.353 | +39.571 |

### Best sampled preparation using available career controls

Balanced/primary seed, downforce.5, one/two stops. Includes original timings and gear-only controls; does not optimize all integer allocations, ratios or pit windows.

| Circuit | Preparation / recipe | Gear | Total s |
|---|---|---:|---:|
| MONZA | cooling / SM-50 / gear-short | 0.35 | 7357.743 |
| SILVERSTONE | cooling / SM-50 / gear-long | 0.85 | 7723.667 |
| ZANDVOORT | cooling / SM-50 / gear-short | 0.35 | 8306.725 |

# Selected thermal policy — era-I/II qualification

Scope complete: **True**. Policy `rolling-heat-065`; 100 kg initial fuel, unchanged coefficients, no AI fit or runtime adoption.

| Era | Main GP | Fresh repeats exact | Laps | Numerical pass | Review flags |
|---|---:|---:|---:|---|---|
| era1 | 120 | 6/6 | 7434 | True | none |
| era2 | 237 | 6/6 | 15918 | True | none |

## era1 — cooling and concentrated preparation

Same recipe set and means for each profile. Positive gap means the concentrated engine/chassis or zero-extra-cooling profile is slower. Exit Cooling1 is retained; a negative exit gap is legitimate. These are sampled alternatives, not global setup optima.

| Circuit | Stage | Concentrated minus best other s | Legal SM gears comparison s |
|---|---|---:|---:|
| MONZA | entry | +171.665 | +169.146 |
| SILVERSTONE | entry | +130.243 | +119.748 |
| ZANDVOORT | entry | +134.449 | +121.269 |

## era1 — sampled balanced stop schedules

Service-inclusive totals; zero stops are diagnostic only. Recipes and their exact boundaries are retained in the archive. One/two/three-stop winners may use different compound families; matched-MH controls are additionally present.

| Circuit | Stage | Best 0-stop s | Best 1-stop s | Best 2-stop s | Best 3-stop s | Three beats one/two |
|---|---|---:|---:|---:|---:|---|
| MONZA | entry | 7476.008 | 7430.525 | 7459.820 | 7532.251 | False |
| SILVERSTONE | entry | 7887.777 | 7795.883 | 7817.685 | 7898.604 | False |
| ZANDVOORT | entry | 8546.677 | 8390.593 | 8396.138 | 8487.962 | False |

## era1 — balanced SM thermal response

Driving-time-weighted means/exposure exclude the first three laps of each stint. Peaks and engine minima include every lap. Surface and carcass are separate model states, not identified tire measurements.

| Circuit | Stage | Surface/carcass mean °C | Hot/cold/window % | Mean temp grip | Max wear % | Min engine capacity |
|---|---|---:|---:|---:|---:|---:|
| MONZA | entry | 86.3/94.0 | 0.0/60.5/39.5 | 0.939 | 67.6 | 0.860 |
| SILVERSTONE | entry | 86.5/93.7 | 0.0/58.5/41.5 | 0.940 | 75.3 | 0.858 |
| ZANDVOORT | entry | 87.0/93.5 | 0.0/58.0/42.0 | 0.943 | 87.4 | 0.853 |

## era1 — legal slider, seed and driver contrasts

| Circuit | Stage | Variant | Delta balanced s | Recoverable lapses |
|---|---|---|---:|---:|
| MONZA | entry | reserved-seed | +1.845 | 3 |
| MONZA | entry | gear-0.35 | -11.640 | 1 |
| MONZA | entry | gear-0.85 | +21.767 | 1 |
| SILVERSTONE | entry | reserved-seed | +1.880 | 3 |
| SILVERSTONE | entry | gear-0.35 | -1.477 | 1 |
| SILVERSTONE | entry | gear-0.85 | +0.983 | 1 |
| ZANDVOORT | entry | reserved-seed | +2.925 | 3 |
| ZANDVOORT | entry | gear-0.35 | -0.588 | 1 |
| ZANDVOORT | entry | gear-0.85 | +43.145 | 1 |
| MONZA | entry | manage | +64.983 | 0 |
| MONZA | entry | attack | -50.158 | 2 |
| SILVERSTONE | entry | manage | +75.294 | 0 |
| SILVERSTONE | entry | attack | -66.723 | 2 |
| ZANDVOORT | entry | manage | +104.389 | 0 |
| ZANDVOORT | entry | attack | -85.380 | 2 |

## era2 — cooling and concentrated preparation

Same recipe set and means for each profile. Positive gap means the concentrated engine/chassis or zero-extra-cooling profile is slower. Exit Cooling1 is retained; a negative exit gap is legitimate. These are sampled alternatives, not global setup optima.

| Circuit | Stage | Concentrated minus best other s | Legal SM gears comparison s |
|---|---|---:|---:|
| MONTREAL | entry | +241.154 | n/a |
| MONTREAL | exit | +17.152 | n/a |
| SPIELBERG | entry | +239.141 | n/a |
| SPIELBERG | exit | +8.779 | n/a |
| MEXICO | entry | +192.626 | n/a |
| MEXICO | exit | +1.894 | n/a |
| SAOPAULO | entry | +203.409 | n/a |
| SAOPAULO | exit | +2.958 | n/a |
| SILVERSTONE | entry | +177.843 | n/a |
| SILVERSTONE | exit | -7.853 | n/a |
| MONZA | entry | +223.501 | n/a |
| MONZA | exit | -9.310 | n/a |
| ZANDVOORT | entry | +174.263 | n/a |
| ZANDVOORT | exit | -7.710 | n/a |

## era2 — sampled balanced stop schedules

Service-inclusive totals; zero stops are diagnostic only. Recipes and their exact boundaries are retained in the archive. One/two/three-stop winners may use different compound families; matched-MH controls are additionally present.

| Circuit | Stage | Best 0-stop s | Best 1-stop s | Best 2-stop s | Best 3-stop s | Three beats one/two |
|---|---|---:|---:|---:|---:|---|
| MONTREAL | entry | not sampled | 8152.198 | 8173.310 | 8427.981 | False |
| MONTREAL | exit | 6938.042 | 6915.638 | 6917.771 | 6928.929 | False |
| SPIELBERG | entry | not sampled | 7861.205 | 7885.764 | 8126.442 | False |
| SPIELBERG | exit | 6674.225 | 6660.924 | 6664.573 | 6678.803 | False |
| MEXICO | entry | not sampled | 8768.880 | 8775.292 | 9086.126 | False |
| MEXICO | exit | 7609.049 | 7547.594 | 7547.421 | 7555.956 | False |
| SAOPAULO | entry | not sampled | 7968.384 | 7988.383 | 8275.412 | False |
| SAOPAULO | exit | 6935.672 | 6855.233 | 6832.636 | 6852.270 | False |
| SILVERSTONE | entry | not sampled | 7785.406 | 7807.594 | 8051.927 | False |
| SILVERSTONE | exit | 6731.246 | 6684.894 | 6670.306 | 6686.461 | False |
| MONZA | entry | not sampled | 7420.257 | 7449.237 | 7655.768 | False |
| MONZA | exit | 6445.469 | 6421.217 | 6412.206 | 6425.932 | False |
| ZANDVOORT | entry | not sampled | 8378.151 | 8383.947 | 8685.000 | False |
| ZANDVOORT | exit | 7277.172 | 7165.655 | 7132.320 | 7155.988 | False |

## era2 — balanced SM thermal response

Driving-time-weighted means/exposure exclude the first three laps of each stint. Peaks and engine minima include every lap. Surface and carcass are separate model states, not identified tire measurements.

| Circuit | Stage | Surface/carcass mean °C | Hot/cold/window % | Mean temp grip | Max wear % | Min engine capacity |
|---|---|---:|---:|---:|---:|---:|
| MONTREAL | entry | 84.1/90.9 | 0.0/68.1/31.9 | 0.922 | 73.4 | 0.849 |
| MONTREAL | exit | 116.8/130.1 | 62.4/1.2/36.4 | 0.935 | 100.0 | 1.000 |
| SPIELBERG | entry | 84.5/91.6 | 0.0/65.8/34.2 | 0.925 | 69.6 | 0.845 |
| SPIELBERG | exit | 117.7/131.6 | 65.9/1.0/33.1 | 0.928 | 100.0 | 1.000 |
| MEXICO | entry | 84.5/90.8 | 0.0/67.2/32.8 | 0.925 | 83.0 | 0.851 |
| MEXICO | exit | 114.4/126.0 | 44.6/0.9/54.5 | 0.952 | 100.0 | 1.000 |
| SAOPAULO | entry | 86.6/93.5 | 0.0/59.8/40.2 | 0.940 | 78.4 | 0.847 |
| SAOPAULO | exit | 120.7/133.9 | 73.9/0.7/25.5 | 0.906 | 100.0 | 1.000 |
| SILVERSTONE | entry | 86.6/93.8 | 0.0/58.4/41.6 | 0.941 | 75.3 | 0.857 |
| SILVERSTONE | exit | 120.4/134.1 | 74.0/0.0/26.0 | 0.914 | 100.0 | 1.000 |
| MONZA | entry | 86.3/94.0 | 0.0/60.5/39.5 | 0.939 | 67.5 | 0.859 |
| MONZA | exit | 121.5/136.7 | 75.0/0.0/25.0 | 0.904 | 100.0 | 1.000 |
| ZANDVOORT | entry | 87.0/93.6 | 0.0/57.9/42.1 | 0.943 | 87.3 | 0.852 |
| ZANDVOORT | exit | 119.4/131.4 | 72.9/0.6/26.6 | 0.918 | 100.0 | 1.000 |

## era2 — legal slider, seed and driver contrasts

| Circuit | Stage | Variant | Delta balanced s | Recoverable lapses |
|---|---|---|---:|---:|
| MONTREAL | exit | gear-0.35 | -3.498 | 1 |
| MONTREAL | exit | gear-0.85 | -2.578 | 1 |
| MONTREAL | exit | downforce-0.2 | -46.323 | 1 |
| MONTREAL | exit | downforce-0.8 | +78.698 | 1 |
| MONTREAL | exit | reserved-seed | +2.563 | 3 |
| SPIELBERG | exit | gear-0.35 | -5.302 | 1 |
| SPIELBERG | exit | gear-0.85 | +8.363 | 1 |
| SPIELBERG | exit | downforce-0.2 | -47.834 | 1 |
| SPIELBERG | exit | downforce-0.8 | +96.570 | 1 |
| SPIELBERG | exit | reserved-seed | +3.625 | 3 |
| MEXICO | exit | gear-0.35 | +4.092 | 1 |
| MEXICO | exit | gear-0.85 | -13.989 | 1 |
| MEXICO | exit | downforce-0.2 | -29.162 | 1 |
| MEXICO | exit | downforce-0.8 | +62.589 | 1 |
| MEXICO | exit | reserved-seed | +1.827 | 3 |
| SAOPAULO | exit | gear-0.35 | +0.301 | 1 |
| SAOPAULO | exit | gear-0.85 | -2.614 | 1 |
| SAOPAULO | exit | downforce-0.2 | -42.261 | 1 |
| SAOPAULO | exit | downforce-0.8 | +71.427 | 1 |
| SAOPAULO | exit | reserved-seed | -0.809 | 3 |
| SILVERSTONE | exit | gear-0.35 | +1.830 | 1 |
| SILVERSTONE | exit | gear-0.85 | +7.890 | 1 |
| SILVERSTONE | exit | downforce-0.2 | -41.073 | 1 |
| SILVERSTONE | exit | downforce-0.8 | +79.574 | 1 |
| SILVERSTONE | exit | reserved-seed | +1.029 | 3 |
| MONZA | exit | gear-0.35 | -2.896 | 1 |
| MONZA | exit | gear-0.85 | +3.286 | 1 |
| MONZA | exit | downforce-0.2 | -59.403 | 1 |
| MONZA | exit | downforce-0.8 | +96.441 | 1 |
| MONZA | exit | reserved-seed | +1.109 | 3 |
| ZANDVOORT | exit | gear-0.35 | +2.878 | 1 |
| ZANDVOORT | exit | gear-0.85 | -1.037 | 1 |
| ZANDVOORT | exit | downforce-0.2 | -18.579 | 1 |
| ZANDVOORT | exit | downforce-0.8 | +43.532 | 1 |
| ZANDVOORT | exit | reserved-seed | -0.163 | 3 |
| MONTREAL | exit | manage | +24.659 | 0 |
| MONTREAL | exit | attack | -23.285 | 2 |
| SILVERSTONE | exit | manage | +27.939 | 0 |
| SILVERSTONE | exit | attack | -16.181 | 2 |
| MONZA | exit | manage | +26.463 | 0 |
| MONZA | exit | attack | -16.431 | 2 |

## Decision and limits

Candidate supports proceeding to final AI preparation: **True**.

No nine-AI win rate, global optimum, all-budget fairness, broad aggression-risk calibration, industrial accuracy or native/WASM proof is claimed. Point reallocations move multiple axes; early aero can be unavailable. Old results retain their own policy identity. All high-temperature/cold/zero-stop extremes and raw scalar trajectories remain archived; no coefficient change or asymmetric stop limit hides a failure. The game/runtime/catalog and deployed services remain unchanged.

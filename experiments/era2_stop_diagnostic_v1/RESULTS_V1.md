# Era-II stop-count diagnostic V1

Numerical/bridge checks: **PASS**. 44 main GP +4 fresh repeats, 2976 accepted laps. 4 exact repeats; four prior input/result bridges checked. Strategic gate remains **open**.

Scope: developed era-II Craft fixture; 16 discretionary points +8 installed bonuses; balanced reference driver; sliders 0.5; **100 kg initial fuel**, 70°C initial/replacement tires. Monza and São Paulo only. All nominal coefficients remain frozen. Complete receipts, per-lap scalars and counterfactual inputs are archived. No AI fitting, live activation or deployment.

## Matched compound exposure

Within each row the exact number of laps on each compound is unchanged across stop counts; sequence and stint age change. M/H/S mean medium/hard/soft; the first letter receives floor(N/2) laps. Positive gain means faster than one stop. These centered schedules are samples, not optimized strategies.

| Circuit | Family | One stop s | Two stops s | Three stops s | Two-stop gain s | Three-stop gain s | Best stops |
|---|---|---:|---:|---:|---:|---:|---:|
| MONZA | MH | 6543.446 | 6486.791 | 6470.997 | 56.655 | 72.449 | 3 |
| MONZA | HM | 6539.523 | 6532.238 | 6472.776 | 7.285 | 66.747 | 3 |
| MONZA | SM | 6718.248 | 6564.763 | 6507.035 | 153.485 | 211.213 | 3 |
| MONZA | MS | 6715.330 | 6669.283 | 6508.469 | 46.048 | 206.861 | 3 |
| SAOPAULO | MH | 7004.997 | 6918.251 | 6899.211 | 86.746 | 105.786 | 3 |
| SAOPAULO | HM | 6999.373 | 6989.082 | 6898.598 | 10.291 | 100.774 | 3 |
| SAOPAULO | SM | 7229.584 | 7038.537 | 6952.569 | 191.047 | 277.015 | 3 |
| SAOPAULO | MS | 7226.895 | 7149.736 | 6952.516 | 77.159 | 274.379 | 3 |

## Best sampled nominal plans, including timing controls

| Circuit | Stops | Plan | Driving s | Service s | Total s |
|---|---:|---|---:|---:|---:|
| MONZA | 1 | MONZA/HM/one/nominal | 6517.523 | 22.000 | 6539.523 |
| MONZA | 2 | MONZA/MH/two/nominal | 6442.791 | 44.000 | 6486.791 |
| MONZA | 3 | MONZA/MH/three/nominal | 6404.997 | 66.000 | 6470.997 |
| SAOPAULO | 1 | SAOPAULO/HM/one/nominal | 6977.373 | 22.000 | 6999.373 |
| SAOPAULO | 2 | SAOPAULO/MH/two/nominal | 6874.251 | 44.000 | 6918.251 |
| SAOPAULO | 3 | SAOPAULO/HM/three/nominal | 6832.598 | 66.000 | 6898.598 |

Each stop still costs 22 s, including the diagnostic counterfactuals. Early/late two-stop controls retain exact compound totals; their individual records and stint boundaries are in `DECISION_V1.json` and the archive.

## One-factor counterfactuals: MH family

Only the selected multiplicative grip penalty is removed. Heat laws, wear generation, mount preparation and service cost remain. Trajectories respond to the new grip: differences cannot be added as independent causal seconds.

| Circuit | Diagnostic | Two-stop gain vs one s | Three-stop gain vs one s |
|---|---|---:|---:|
| MONZA | nominal | 56.655 | 72.449 |
| MONZA | no_temperature_grip_loss | -15.107 | -36.564 |
| MONZA | no_wear_grip_loss | 42.468 | 54.794 |
| SAOPAULO | nominal | 86.746 | 105.786 |
| SAOPAULO | no_temperature_grip_loss | -7.843 | -30.232 |
| SAOPAULO | no_wear_grip_loss | 53.185 | 64.279 |

## Nominal MH stint trajectories

Surface/carcass means are weighted by driving time. Wear/grip are model fractions, not measured tire identification. Minimum temperature grip is the applied surface-temperature factor; carcass temperatures retain thermal memory.

| Circuit | Stops | Laps | Tire | Surface mean/max °C | Carcass mean °C | End wear % | End wear grip | Min temp grip |
|---|---:|---|---|---:|---:|---:|---:|---:|
| MONZA | 1 | 1–26 | medium | 124.5/140.4 | 140.4 | 76.0 | 0.925 | 0.751 |
| MONZA | 1 | 27–53 | hard | 121.3/138.3 | 137.2 | 46.3 | 0.982 | 0.692 |
| MONZA | 2 | 1–13 | medium | 111.2/134.0 | 124.9 | 32.2 | 0.992 | 0.753 |
| MONZA | 2 | 14–40 | hard | 122.0/139.0 | 137.8 | 47.3 | 0.982 | 0.691 |
| MONZA | 2 | 41–53 | medium | 109.7/131.8 | 123.2 | 30.4 | 0.992 | 0.753 |
| MONZA | 3 | 1–13 | medium | 111.2/134.0 | 124.9 | 32.2 | 0.992 | 0.753 |
| MONZA | 3 | 14–26 | hard | 107.5/130.6 | 120.5 | 21.5 | 0.995 | 0.691 |
| MONZA | 3 | 27–39 | medium | 110.3/132.6 | 123.9 | 31.2 | 0.992 | 0.753 |
| MONZA | 3 | 40–53 | hard | 108.2/130.9 | 121.4 | 22.5 | 0.995 | 0.692 |
| SAOPAULO | 1 | 1–35 | medium | 124.4/138.1 | 138.3 | 90.6 | 0.861 | 0.747 |
| SAOPAULO | 1 | 36–71 | hard | 121.4/137.7 | 135.4 | 55.4 | 0.972 | 0.687 |
| SAOPAULO | 2 | 1–17 | medium | 111.5/134.3 | 123.5 | 37.8 | 0.989 | 0.747 |
| SAOPAULO | 2 | 18–53 | hard | 122.1/138.4 | 136.2 | 56.7 | 0.970 | 0.687 |
| SAOPAULO | 2 | 54–71 | medium | 110.8/132.8 | 122.9 | 37.9 | 0.989 | 0.747 |
| SAOPAULO | 3 | 1–17 | medium | 111.5/134.3 | 123.5 | 37.8 | 0.989 | 0.747 |
| SAOPAULO | 3 | 18–35 | hard | 108.9/131.9 | 120.5 | 26.5 | 0.994 | 0.687 |
| SAOPAULO | 3 | 36–53 | medium | 111.5/133.7 | 123.7 | 38.8 | 0.988 | 0.747 |
| SAOPAULO | 3 | 54–71 | hard | 107.7/130.3 | 119.2 | 25.6 | 0.994 | 0.687 |

## Evidence and limits

- Native source `ecd7c801c3169fb2cbbc19d6d8c6f0710bcc817e`; executable SHA256 `4025a03ccb89b5c94dedde5448418386ceb1b62a6437e4bd5970b323cd06dfba`.
- Four nominal MH-half/SM-half runs reproduce the archived physical-input and complete-result digests exactly. Four repeated runs match too. This checks experiment continuity and deterministic execution, not physical realism.
- Maximum tire balance residual 6.69e-09 J; elapsed residual 9.09e-13 s; lap-boundary carry jump 0.
- No coefficient fit, hard stop limit or final nine-AI calibration. No assertion about untested circuits, budgets or global optima.
- A separate interpretation and next hypothesis are documented in `INTERPRETATION.md`. Numerical pass does not close the strategic gate.

# Thermal balance sensitivity V1

Development: 40 full GP. Selected policy: **rolling-heat-065**.

All cases start with 100 kg fuel and tires at 70°C. Nominal operating windows, grip/wear laws, service costs, engine and sampling remain unchanged. Exposure/grip means exclude the first three laps of each stint; all laps remain in the archived race totals. These are reduced-model screening hypotheses, not measured tire coefficients.

| Candidate | Eligible | Relative hot-exposure score (lower better) | Rejections |
|---|---|---:|---|
| rolling-heat-065 | True | 1.3321 | none |
| surface-air-120 | True | 1.4534 | none |
| carcass-air-45 | False | 1.2648 | craft_engineering:era1-entry/MONZA/1:entry_grip_loss; craft_engineering:era1-entry/SILVERSTONE/1:entry_grip_loss; craft_engineering:era2-entry/MONZA/1:entry_grip_loss; craft_engineering:era2-entry/MONTREAL/1:entry_grip_loss |

## Development thermal measurements

| Candidate | Context | Hot exposure ref → trial % | Temperature grip ref → trial | Surface mean ref → trial °C |
|---|---|---:|---:|---:|
| rolling-heat-065 | craft_engineering:era2-exit/MONZA/1 | 75.5 → 50.5 | 0.880 → 0.945 | 128.3 → 119.4 |
| rolling-heat-065 | craft_engineering:era2-exit/SAOPAULO/1 | 73.5 → 48.8 | 0.886 → 0.943 | 127.0 → 118.9 |
| rolling-heat-065 | craft_engineering:era1-entry/MONZA/1 | 0.0 → 0.0 | 0.965 → 0.939 | 90.9 → 86.3 |
| rolling-heat-065 | craft_engineering:era1-entry/SILVERSTONE/1 | 0.0 → 0.0 | 0.966 → 0.940 | 91.1 → 86.5 |
| rolling-heat-065 | craft_engineering:era2-entry/MONZA/1 | 0.0 → 0.0 | 0.965 → 0.939 | 90.9 → 86.3 |
| rolling-heat-065 | craft_engineering:era2-entry/MONTREAL/1 | 0.0 → 0.0 | 0.951 → 0.922 | 88.4 → 84.1 |
| surface-air-120 | craft_engineering:era2-exit/MONZA/1 | 75.5 → 55.9 | 0.880 → 0.941 | 128.3 → 120.5 |
| surface-air-120 | craft_engineering:era2-exit/SAOPAULO/1 | 73.5 → 52.4 | 0.886 → 0.941 | 127.0 → 119.7 |
| surface-air-120 | craft_engineering:era1-entry/MONZA/1 | 0.0 → 0.0 | 0.965 → 0.940 | 90.9 → 86.5 |
| surface-air-120 | craft_engineering:era1-entry/SILVERSTONE/1 | 0.0 → 0.0 | 0.966 → 0.942 | 91.1 → 86.7 |
| surface-air-120 | craft_engineering:era2-entry/MONZA/1 | 0.0 → 0.0 | 0.965 → 0.940 | 90.9 → 86.5 |
| surface-air-120 | craft_engineering:era2-entry/MONTREAL/1 | 0.0 → 0.0 | 0.951 → 0.925 | 88.4 → 84.4 |
| carcass-air-45 | craft_engineering:era2-exit/MONZA/1 | 75.5 → 51.2 | 0.880 → 0.946 | 128.3 → 119.9 |
| carcass-air-45 | craft_engineering:era2-exit/SAOPAULO/1 | 73.5 → 43.1 | 0.886 → 0.948 | 127.0 → 118.8 |
| carcass-air-45 | craft_engineering:era1-entry/MONZA/1 | 0.0 → 0.0 | 0.965 → 0.933 | 90.9 → 85.5 |
| carcass-air-45 | craft_engineering:era1-entry/SILVERSTONE/1 | 0.0 → 0.0 | 0.966 → 0.934 | 91.1 → 85.5 |
| carcass-air-45 | craft_engineering:era2-entry/MONZA/1 | 0.0 → 0.0 | 0.965 → 0.933 | 90.9 → 85.5 |
| carcass-air-45 | craft_engineering:era2-entry/MONTREAL/1 | 0.0 → 0.0 | 0.951 → 0.915 | 88.4 → 83.1 |

## Strategy comparisons

Complete elapsed times include every 22 s service. These centered recipes have equal compound exposure within each context; they are not global optima.

| Phase | Policy | Circuit | Stops | Driving s | Service s | Total s |
|---|---|---|---:|---:|---:|---:|
| development | reference | MONZA | 1 | 6521.446 | 22.000 | 6543.446 |
| development | reference | MONZA | 2 | 6442.791 | 44.000 | 6486.791 |
| development | reference | MONZA | 3 | 6404.997 | 66.000 | 6470.997 |
| development | reference | SAOPAULO | 1 | 6982.997 | 22.000 | 7004.997 |
| development | reference | SAOPAULO | 2 | 6874.251 | 44.000 | 6918.251 |
| development | reference | SAOPAULO | 3 | 6833.211 | 66.000 | 6899.211 |
| development | rolling-heat-065 | MONZA | 1 | 6399.217 | 22.000 | 6421.217 |
| development | rolling-heat-065 | MONZA | 2 | 6368.206 | 44.000 | 6412.206 |
| development | rolling-heat-065 | MONZA | 3 | 6368.793 | 66.000 | 6434.793 |
| development | rolling-heat-065 | SAOPAULO | 1 | 6833.233 | 22.000 | 6855.233 |
| development | rolling-heat-065 | SAOPAULO | 2 | 6788.636 | 44.000 | 6832.636 |
| development | rolling-heat-065 | SAOPAULO | 3 | 6794.479 | 66.000 | 6860.479 |
| development | surface-air-120 | MONZA | 1 | 6407.582 | 22.000 | 6429.582 |
| development | surface-air-120 | MONZA | 2 | 6375.453 | 44.000 | 6419.453 |
| development | surface-air-120 | MONZA | 3 | 6373.510 | 66.000 | 6439.510 |
| development | surface-air-120 | SAOPAULO | 1 | 6839.907 | 22.000 | 6861.907 |
| development | surface-air-120 | SAOPAULO | 2 | 6795.183 | 44.000 | 6839.183 |
| development | surface-air-120 | SAOPAULO | 3 | 6798.997 | 66.000 | 6864.997 |
| development | carcass-air-45 | MONZA | 1 | 6398.070 | 22.000 | 6420.070 |
| development | carcass-air-45 | MONZA | 2 | 6368.887 | 44.000 | 6412.887 |
| development | carcass-air-45 | MONZA | 3 | 6368.605 | 66.000 | 6434.605 |
| development | carcass-air-45 | SAOPAULO | 1 | 6820.054 | 22.000 | 6842.054 |
| development | carcass-air-45 | SAOPAULO | 2 | 6783.485 | 44.000 | 6827.485 |
| development | carcass-air-45 | SAOPAULO | 3 | 6791.392 | 66.000 | 6857.392 |
| reserved | reference | MEXICO | 1 | 7614.379 | 22.000 | 7636.379 |
| reserved | reference | MEXICO | 2 | 7551.381 | 44.000 | 7595.381 |
| reserved | reference | MEXICO | 3 | 7546.511 | 66.000 | 7612.511 |
| reserved | rolling-heat-065 | MEXICO | 1 | 7525.594 | 22.000 | 7547.594 |
| reserved | rolling-heat-065 | MEXICO | 2 | 7503.421 | 44.000 | 7547.421 |
| reserved | rolling-heat-065 | MEXICO | 3 | 7533.961 | 66.000 | 7599.961 |

## Frozen reserved validation

Screening PASS: **True**; exact repeats 6/6. Rejection reasons: none.

The candidate is not admitted to a runtime or deployed game. Broader era-I/II qualification, AI fitting, WASM/recovery and release admission remain separate gates.

Native development source `d7f56da0b79c063e51d1455eab3ae4a6c0e8c584`; binary `3d745b32eb71eb22a3046ce7010909590242e1f4eb6df3151eee73bbd18c44ed`. Reference bridges and all candidate acceptance/rejection reasons are in `selection-v1.json`; raw receipts, scalar trajectories and conservation checks are retained in the phase archives.

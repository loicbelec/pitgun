# Native results — 25 November 2026

Verdict: **PASS for the bounded numerical/compilation gate; REFINE for physical identification and competitive admission.** No existing game runtime selects this policy.

## What changed

Three fixed references (soft/C5, medium/C4, hard/C3), explicit95%-of-temperature-peak bands and revised heat retention/exchange within the existing two-capacity solver. No replacement equation, extra heat source, temperature clamp or sampling-rate change. Engine cooling, gearbox, driver traits and AI preparations are unchanged.

## Five-lap surface response

Time-weighted surface temperature during lap5; actual carcass is a separate state. Identical corrected-gear player setup and compound for each baseline/candidate pair.

| Context | Compound | Baseline surface °C | Candidate surface °C | Candidate carcass °C | Temperature grip factor |
|---|---|---:|---:|---:|---:|
| MONZA / era1 | soft | 68.6 | 84.6 | 92.3 | 0.947 |
| MONZA / era1 | medium | 68.2 | 83.3 | 90.5 | 0.898 |
| MONZA / era1 | hard | 67.8 | 82.4 | 89.1 | 0.840 |
| MONTREAL / era2 | soft | 78.9 | 101.4 | 112.1 | 0.999 |
| MONTREAL / era2 | medium | 77.3 | 97.7 | 107.4 | 0.987 |
| MONTREAL / era2 | hard | 76.0 | 94.7 | 103.6 | 0.948 |

All six normal candidate traces include increasing and decreasing segments. Lap-average temperature can rise while the surface cools on individual straights. The cold-start scenarios heat; hot-start cases cool. This is not a forced monotonic warm-up.

## Long distance exposes the first hypothesis

Both ordinary complete-GP references use a prescribed soft/medium one-stop plan, not an optimized strategy. The table shows the last-lap mean; it is not an assertion about peak local tread temperature.

| Context | High-retention surface °C | Retained candidate surface °C | Candidate carcass °C | Final wear |
|---|---:|---:|---:|---:|
| MONZA / era1 | 102.7 | 93.0 | 102.4 | 46.8% |
| MONTREAL / era2 | 141.8 | 129.3 | 144.8 | 73.3% |

The high-retention hypothesis fails the declared normal long-distance140°C surface gate at Montreal. It is retained, not silently discarded. The reduced-retention/increased-air-exchange candidate passes that bounded gate. Montreal remains hot and loses grip; there is no attempt to keep every setup inside its peak band. No further tuning occurred after the reserved-context runs.

## Reserved context

Silverstone-II lap5 surface means (not used to choose the candidate):
- soft: 109.2°C.
- medium: 105.2°C.
- hard: 102.0°C.

## Numerical evidence

- Per policy:41 cases +6 exact repeats =47 serial player solves /348 driven laps. Two final pinned reports retain94 solves in total.
- All candidate repeated result digests exact. Maximum accepted-segment tire energy residual 5.17e-09J; global generated+direct-environment = rolling+deformation within1e-5J.
- Every native lap boundary carries both temperatures exactly. New sets use70°C on both nodes; unchanged laps do not reset.
- 242 scoped Rust checks pass:151 simulator unit,31 solver unit,60 physical-kernel integration. Formatting/diff and strict production-library/probe Clippy pass. A broader test-target Clippy run still reports two existing warnings in unrelated test helpers; the new test warning was fixed.

- Executable source: `4564e456186444785612a47e6b89ffc7f10a0642`; binary SHA256 `01d7253fc96fca10942933d92a150cca51615b9ee38b8c1e95db77bb5228ddad`.
- Both reports assert unchanged source/policy/binary during execution. They record per-file hashes, complete policy and input/result digests. The second run has an untracked first report; its numerical source hashes remain identical.

## Remaining limits before game activation

- Early low-load Monza medium/hard warm-up remains slow: medium lap means69.9,74.3,77.9,80.9,83.3°C. Do not describe these five laps as already inside the near-peak band. Assess the time constants/source partition and the out-lap cost rather than lifting displayed values.
- Montreal late carcass is about145°C in this reference. This is a reduced aggregate state, not measured IR tread or an industrial validation; inspect setup/load/driver sensitivity and usable operating limits.
- Compare zero/one/two stops plus a three-stop counterexample on full GP, with real pit loss and all three compounds. These two prescribed one-stop examples do not prove optimal stop count or balance.
- Qualify driver/setup response and budget-aware nine-AI fairness after physics freeze; no AI setup was selected from this dataset.
- A new explicit model/catalog/artifact and native/WASM/recovery/admission checks are still required before a separate local game preview. Existing5251, staging, production and leaderboard verification behavior are unchanged.

Approved next dependencies are recorded in [the delivery sequence](../../docs/physics/DELIVERY_2026-11-25.md), #479 and selective-verification #480.

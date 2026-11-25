# Selected thermal policy qualification

Tracking [#487](https://github.com/loicbelec/pitgun/issues/487), under #479/#483, following [the bounded screen](../thermal_balance_sensitivity_v1/INTERPRETATION.md). Read [ACCEPTANCE.md](ACCEPTANCE.md) for the frozen matrix and review rules. This qualifies the separately hashed `rolling-heat-065` policy; it does not alter the existing game.

## Reuse and isolation

No new Rust solver or probe is introduced. The runner reuses `thermal_balance_sensitivity_probe` and the frozen screen harness/checker. It imports the previous era-I/II matrix generators without modifying them, replaces unavailable era-I paired sliders with legal gear controls, and adds explicitly declared strategy controls. A wrapper pin binds this orchestration, its reporting/tests/protocol and every imported source/plan/archive in addition to the native pin. Each case is freshly solved; earlier screen evidence is used for exact bridges, not presented as a fresh run.

All eras share one worker-lock parent. Run era I, inspect/checkpoint it, then era II. Each native result is cached atomically under the case hash. Restarting the same command resumes exact entries; executable/source/policy mismatches require a fresh namespace. Keep phase caches outside the checkout and commit sources before starting. One native process runs at a time, compilation uses two jobs.

## Reproduce

Use clean pinned sources and fresh absolute cache directories:

```sh
python3 -m unittest discover -s experiments/thermal_candidate_qualification_v1 -v
python3 experiments/thermal_candidate_qualification_v1/run.py --era era1 --target-dir /ABSOLUTE/TARGET --cache /ABSOLUTE/CAMPAIGN/era1 --output /ABSOLUTE/CAMPAIGN/era1.json.gz
python3 experiments/thermal_candidate_qualification_v1/report.py /ABSOLUTE/CAMPAIGN/era1.json.gz --output-dir /ABSOLUTE/CAMPAIGN/era1-report
python3 experiments/thermal_candidate_qualification_v1/run.py --era era2 --target-dir /ABSOLUTE/TARGET --cache /ABSOLUTE/CAMPAIGN/era2 --output /ABSOLUTE/CAMPAIGN/era2.json.gz
python3 experiments/thermal_candidate_qualification_v1/report.py /ABSOLUTE/CAMPAIGN/era1.json.gz /ABSOLUTE/CAMPAIGN/era2.json.gz --output-dir /ABSOLUTE/CAMPAIGN/final-report
```

Native sources are unchanged from the bounded screen; its two scoped Rust tests already cover fixture resolution and invalid means/controls. Five new orchestration tests cover counts, actual means, seven-circuit calendar union, legal sliders, exact-exposure integer stint boundaries and all fourteen semantic screen matches before solving. The report recalculates numerical checks, repeat digests and review flags from retained native evidence. It cannot certify a partial matrix or a single era as full scope.

The native checker verifies actual 100 kg initial fuel, components and budget/bonus floors, finish/service accounting, state carry, exact pit preparation, thermal ledger closure and exposure accounting. Original plan prose saying 110 kg is historical and is not used as an input. No invalid legacy-only boundary switch or unavailable downforce setting is submitted.

## Interpretation limits

Normal point-profile comparisons use the same recipe set. Balanced-only added controls are not quietly used to give one point profile a larger search space. Stop-count comparisons include different compound families and selected timings; matched-MH schedules are separately available, but no global optimum is claimed. Zero-stop runs are physical diagnostics. A three-stop alternative can be valid; only universal sampled dominance is a declared review flag.

This is player-only native evidence at selected actual means: era-I entry (eight discretionary points), era-II entry/exit (11/16 discretionary points plus installed bonuses). It is not a sweep of every intermediate budget or era-I exit, all three doctrine bonus trajectories, a nine-AI race, broad stochastic risk calibration or measured tire identification. Those limits remain important before runtime admission. No remote service, provider or leaderboard is called; no saved career, runtime, catalog or deployment is changed.

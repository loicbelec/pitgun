# Era-I strategic qualification

**Fuel-scope correction:** retained runs start with100kg. The110kg wording in the original acceptance/report prose is incorrect; only100kg was qualified. See [the explicit erratum](../three_compound_thermal_v1/FUEL_SCOPE_ERRATUM_V1.md).
Follow-up to #479 / draft#481, after the owner explicitly accepted circuit-dependent tire behavior. No physical coefficient change. [Preregistered matrix and interpretation](ACCEPTANCE.md).

Run one native process at a time, with at most two compiler jobs:

```
python3 experiments/era1_strategy_qualification_v1/run.py \
  --target-dir /ABSOLUTE/LOCAL/CARGO/TARGET \
  --cache /ABSOLUTE/PRIVATE/NEW/CACHE \
  --output /ABSOLUTE/PRIVATE/CAMPAIGN.json.gz
```

`--limit 1` runs only a harness preflight and leaves an explicitly incomplete checkpoint. Re-running resumes immutable cached cases. A source, executable, plan or tire-policy mismatch refuses the cache; use a new namespace. Cache receipt contains full physical input digest and preparation; reports contain per-lap scalar measures, not full telemetry archives.

114 main cases +12 validation executions. C3/C4/C5 identities and current heat/wear parameters stay frozen. Three effective-point profiles and coarse strategy recipes, not an allocation/strategy search. Mode and reserved-seed comparisons are explicitly separated. The nine AI opponents are not simulated or fitted; preparation resolves their existing catalog context only because the current game bridge shares that boundary.

No release/model switch, WASM rebuild, hosted verification, provider calls or deployment is performed by this experiment.

## Available career controls versus physical diagnostics

The baseline `none` aero component has no adjustable_downforce; the game clamps it to.5. The original18paired downforce/gearing cases therefore remain offline physical sensitivity diagnostics, never legal era-I career recommendations. Original0/3-stop cases similarly do not change ranked/session sporting rules.

A post-plan confound control is separately frozen before execution by `gear_controls.py`:18SM runs, all3point profiles x3circuits xgear.35/.85, downforce kept.5. The cases derive from original definitions, not selected results. Its manifest, runner hash and new immutable cache namespace are retained separately; no primary plan or coefficients are changed. It reuses the exact native executable.

## Validation correction

The frozen plan mistakenly requested the legacy invariant-work switch in6ReachableWearV1 cases. The solver explicitly rejects this combination before any lap; all6rejections stay in the primary dataset. This is a harness error, not a physical failure or a result mismatch. `validation_controls.py` separately freezes and repeats those same6physical cases using the supported mode, in addition to the original6exact repeats. No alternative boundary-algorithm equivalence is claimed. Primary plan/results are preserved unchanged.

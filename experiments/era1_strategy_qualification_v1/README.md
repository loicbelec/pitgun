# Era-I strategic qualification

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

# Racing Model V3 thermal family refinement

This experiment implements the pre-registered refinement gate from issue
`#296`. It does not modify the game, a published Racing Catalog, Authority or
Verifier.

The reviewed `adaptive-038` response is retained unchanged for F1 2026. The
local screen changes only the modern V6T thermal soft-limit offset. This is a
direct, unit-bearing threshold in degrees Celsius, so each candidate remains
physically interpretable and every other thermal coefficient stays fixed.

The screen deliberately permits severe derating at zero cooling. Such a setup
is a bad player choice, not a numerical pathology, provided that the result is
finite, stays below the hard temperature bound and recovers monotonically when
cooling is added. The desired game response is an attainable interior optimum:
ten cooling points must beat both zero and twenty points.

Build the immutable Rust probe and run the local selection only after the
contract and runner have been reviewed:

```bash
cargo build --release -p pitgun-racing-simulator \
  --example v3_decision_surface_probe
python3 experiments/racing_v3_thermal_refinement/screen_local.py --jobs 12
```

Silverstone, seed `20260901` and the full-race workload are reserved for the
later Databricks validation and are rejected by the local runner.

## Local result

The immutable local screen completed 126/126 executions. Only
`soft-limit--3.0c` passed every circuit/seed triplet, so the local verdict is
`PASS` and this parameter set advances to independent validation. No catalog
or game change is authorized by that result.

The selected response is intentionally severe at zero cooling, particularly
at Monza, but remains finite, below 115 °C in the observed selection points
and fully recovers at ten cooling points. Twenty points are slower than ten on
all reviewed circuits because their additional drag exceeds their thermal
benefit.

The detailed interpretation is documented in
[`RACING_V3_THERMAL_REFINEMENT_LOCAL_V1.md`](../../docs/RACING_V3_THERMAL_REFINEMENT_LOCAL_V1.md).

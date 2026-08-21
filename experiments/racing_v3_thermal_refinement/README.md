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

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

## Independent validation and candidate

The reserved Databricks validation completed 12/12 executions and recorded 96
metrics. A separate read-only review reproduced the evidence from the pinned
Delta snapshots and returned `PASS` for `historical_v8`, `modern_v6t` and
`f1_2026`.

That gate authorizes the reviewed candidate
`pitgun.racing-v3-thermal-family-profile@1.0.0-rc.1`. Rebuild it with:

```bash
python3 experiments/racing_v3_thermal_refinement/build_candidate.py
```

The generated JSON is selected by `vehicle_id`, fails closed for unknown
vehicles and contains only the thermal slice. In particular, the experimental
130 kg fuel reservoir is excluded: the production fuel and hybrid-energy
contract remains owned by issue `#246`.

The candidate authorizes the next Rust/WASM integration step. It does not by
itself authorize catalog publication, Authority/Verifier promotion, game
staging or production. See
[`RACING_V3_THERMAL_FAMILY_PROFILE_CANDIDATE_V1.md`](../../docs/RACING_V3_THERMAL_FAMILY_PROFILE_CANDIDATE_V1.md)
for the complete release boundary.

## Power-unit binding candidate

Component composition changes the selection semantics without changing the
reviewed coefficients above. The versioned V2 candidate therefore binds each
thermal family to stable installed power-unit identities (`v8_1960`,
`v8_1970`, `v6t`, `v6t_hybrid`) rather than to monolithic vehicle identities.
It targets the distinct non-production Model V3 `0.11.0` identity and records
the resolved family for every competitor.

The exact candidate and checksum are:

- `candidates/thermal-family-profile-v2.json`;
- `candidates/thermal-family-profile-v2.sha256`.

It remains an integration candidate. Catalog publication and hosted service
promotion are intentionally deferred so Catalog `1.5.0` and Model `0.10.0`
retain their exact replay semantics.

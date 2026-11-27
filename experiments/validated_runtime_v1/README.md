# Local I/II runtime bridge

This integrates the frozen `fixed_nine_fresh_reserve_v1` candidate into a new, unpublished Racing catalog1.10.3/model0.16.3. It does not alter coefficients, driver traits, previous catalogs or their LATEST pointer. The materialized table contains preparations, never race results.

`bridge-inputs-v1.json` retains327 distinct original physical-input fingerprints and two complete GP references. `gear-ratios-native-v1.json` freezes the native base spacing to remove a one-ULP target-libm difference; these327 native inputs remain unchanged. Exact Rust/WASM request preparation and fingerprint agreement are checked by the game integration harness.

`game-budget-envelope-v1.json` pins the actual game development/offset sources and conservative optional-HQ bounds. The reserve campaign qualified I8–12 and II12–24, not every optional development combination. The runtime table covers every integer I8–27 and II8–53. Outside the qualified range it projects the nearest frozen endpoint allocation to the exact cap, preserving its gear/downforce/stint plan and per-axis bound20. This is local technical coverage, **not a claim of additional balance qualification** or immediate affordability of every development. Inputs never depend on player spending or results; effective bonuses are counted once.

Generate with `python3 scripts/prepare-validated-career-profile.py`; catalog generation and compiled snapshot checks must follow the repository catalog workflow. Build from a clean commit using the game helper `scripts/control/prepare-three-compound-runtime.mjs FRAMEWORK`, with an existing shared CARGO_TARGET_DIR. New artifacts are content-addressed and immutable.

`cargo run --offline --locked --release -p pitgun-racing-simulator --example validated_career_bridge` resolves all retained inputs without new GP simulations. The `limits` argument performs two technical full-race smoke tests at I27/Monza and II53/Mexico. Their timings are not training data. A vector ID argument emits resolved inputs for diagnostics.

Runtime tests cover all integer table budgets, the12 actual doctrine entry/exit player-component snapshots, unspent-point independence, unsupported caps, explicit gear spacing, and exact same-artifact changed-pit replay. Game evidence lives in `docs/physics/validated-career-local-qa-v1.json` of the game integration branch.

Signed Authority/Verifier admission, provider access, public catalog publication and deployments are intentionally absent. Later III/IV/V lots must use actual era components and their own joint preparation/physics evaluation. Existing failed reserves remain retained.

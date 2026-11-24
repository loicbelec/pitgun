# Qualified career I/II · local runtime integration

Historical model 0.16.0 evidence below: owner review subsequently identified a physical first-gear lock. See [gear-threshold correction](../gear_threshold_v1/README.md). Model 0.16.1 retains the old evidence but requires new balancing qualification; do not relabel old native results or timing diagnostics as passing corrected outcomes.

This slice connects the frozen [fixed-nine native candidate](../fixed_opponents_v3/RESULTS_V3.md) to the existing incremental Racing host. It does not recalibrate physics or opponents. The scientific campaign and its original binary remain in their separate worktree/archive.

## Identities and scope

- Local candidate model `pitgun.racing-v3-candidate@0.16.0`, catalog `1.10.0`; public `LATEST` and all earlier catalogs stay unchanged.
- Service-control policy `3.0.0` binds the exact qualified profile and energy-power projection. Execution resolution V5 identifies that profile in addition to the baseline resources used during resolution.
- Only the recorded I/II circuit contexts, complete GP distances or short 1–5-lap practices. Other eras/circuits reject explicitly; no silent physical fallback.
- Same final driver traits, constant era budgets, components, offsets, plans, fuel load and RNG namespaces as the retained native policy. Player preparation remains the player's own. Practices use each rival's recorded opening compound without a mandatory race stop.
- Compiled soft/medium/hard resources are used both initially and for a live pit instruction. Reachable-wear lap boundaries, actual engine/tire states and their native events remain authoritative.

## Telemetry bridge

The old power/telemetry projection deliberately rejects this energy model. The new path therefore projects accepted physical work instead of reconstructing pedal commands. The Solver retains one live lap's accepted segment shaft/wheel/loss powers; they are not serialized into historical Solver result identities or accumulated in terminal history. Availability is the mean held-torque thermal capacity before driver request and gear-shift interruption. Consumption/load is delivered shaft power relative to that capacity. Clutch loss is the actual accepted slip work divided by the shared segment duration.

Geometry and temperatures are sampled for presentation at the existing 5 Hz, without changing physical integration spacing or equations. Pedal/brake command channels are absent because those commands are not recorded. Canonical evidence and independent replay regenerate the same projection; the renderer never supplies power, timing or results.

## Qualification gates

`native-reference-v1.json` retains two sealed field references (Monza I and Montreal II), their source policy, exact physical-input digests, lap times and outcomes. The focused native input test compares all twenty resolved cars against the original campaign without recalculating it.

Additional checks cover fixed-field independence, unsupported scope, actual accepted power/energy accounting, short ten-car streaming, live compound replacement and exact recorded-decision replay. Historical regressions, native/WASM equality or explained numerical differences, browser Worker/cost, signed local Authority/Verifier and gameplay remain separate gates. No hosted admission or deployment is performed by this integration.

The owner playtest must especially inspect the old-technology entry-II transition (all four prior prepared snapshots P10), upgrade affordability, meaningful setup choices and plausible race strategies. Further calibrations proceed locally one era at a time. Hybrid recovery/storage/deployment precedes fitting motorisations requiring it.

## Local bridge qualification (2026-11-24)

The complete Solver results for all twenty reference cars remain byte-identical to the sealed native campaign (`results/native-reference-replay-v1.json`). Native incremental fields also preserve all twenty exported millisecond totals (`results/incremental-native-fields-v1.json`). The qualified request explicitly rejects presentation geometry and pit-loss overrides.

The separate release WASM is pinned to framework commit `599486b9f521169a9191353599751ba334c76a09`, artifact `sha256:0998e1c01354129daedddc2b74a34474bcb43f26a991c36ddad1a9d46d738ac9`, JavaScript bridge `sha256:a2f2ea8d5343bd54d93666e3020adc575fa29db203415dc7b477c7d103be040b`. Subsequent Verifier routing and test/documentation changes do not replace that binary.

`results/native-wasm-timing-v1.json` records the cross-target diagnostic: all nineteen other GP totals and both full player lap vectors agree to the exported millisecond, Pedro's Monza I total is 1 ms lower in WASM, and field order is unchanged. Thus this is not a claim of bit-identical native/WASM floating-point trajectories. Independent interactive verification must still replay the **exact pinned WASM** and compare exact evidence; no timing tolerance was added to the Verifier. The bounded Node run took about 41 s for Monza and 31 s for Montreal, with an observed process high-water RSS of 981,568 KiB (about 959 MiB). These are one-machine diagnostics, not a browser or production performance benchmark.

Signed practice replay now selects qualified model 0.16 explicitly instead of the legacy timeline model. A dedicated regression accepts a native qualified practice and rejects a modified result. The game normalizes the physical request before both practice and race authorization, preserving original game preparation separately for archive validation. Provider calls and hosted publication remain disabled in the local playtest.

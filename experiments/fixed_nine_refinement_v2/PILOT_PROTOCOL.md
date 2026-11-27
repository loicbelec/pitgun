# Equal-preparation Mexico pilot — V2 development protocol

Follow-up to fixed-nine V1 REFINE (#490/#457). This is the first gate of the approved seven-step refinement. Preserve V1's failed reserves and all physical/driver resources. No local-game, model/catalog, hosted service, Authority/Verifier or leaderboard admission changes.

## Frozen matrix: maximum 30 fresh full GPs

Mexico, full71laps, initial fuel100kg, upgraded fixed-era-II equipment (V8 1970/basic aero), floors A/C/E/K2141. Available **effective** caps16 and24 include floors once. Same exact four preparations for the reference(.84/.84/.84), Franz and Charles. All Balanced; retain each named driver's traits and identity-based stream.

The four proposals are a controlled2x2: effective24 point vectors[2,7,10,5] and[2,9,12,1], each at downforce0.4 and0.5. Gear0.5, medium35/hard36, one service after lap35 for every case. Derive cap16 vectors with the existing deterministic rational projection, identically for all actors; do not use different interpolation routes for rivals and reference. Fully materialize all24 requests before compute.

Use seed271828: it is **opened V1 evidence**, not a new holdout. This diagnostic must not be called fresh independent validation. Sources are the V1 original ordinary proposals and the two V1 locally adjusted reference templates. Four proposals x three actors x two caps =24primary GPs. Predetermined fresh repeats for each actor: cap16 concentrated/df0.4 and cap24 cooling/df0.4, adding6GP. Do not enlarge the matrix or fit coefficients from its results.

Two cap16/df0.4 reference inputs and full outcomes must bridge exactly to V1 reserved Mexico receipts. All30cases are freshly solved in this pilot (except resuming their own completed cached executions); do not import old rows as new computations.

## Numerical and fairness gates

Use the exact retained native executable hash from V1, unchanged Rust and selected rolling-heat-065 policy. Source, plan, input matrix, fixtures, driver roster, earlier evidence and executable are pinned. Commit before execution. One native process, serial budgets. Reject changed namespaces or mismatched echoes.

Every GP uses the same V1 numerical checker: complete finish, exact lap carry/pit preparation, elapsed/service closure, tire-energy partitions, effective cap/floors/fuel/policy and frozen traits/streams. All6fresh repeats and2prior input/full-result bridges must be exact. Any numerical/provenance failure blocks the next expensive campaign.

Before and after resolution, confirm equal point vector, gear, downforce, plan, fuel, hardware and physical policies across actors. Driver resource/control/noise differences are deliberate, never hidden power/pace changes. Points, gear and service costs must match requests; do not infer fairness from displayed labels alone.

## Diagnostic interpretation fixed before compute

Report all four service-inclusive times for each actor/cap, cooling-oriented versus concentrated allocation at each slider, engine thermal limitation and tire state. This is a point-allocation tradeoff: C/E and cooling change together; it is not a cooling-only partial derivative. No general human win-rate claim.

The preparation explanation is supported in this opened Mexico scope if:
1. All numerical/provenance/fairness gates pass.
2. The cooling-oriented tradeoff is faster than the concentrated choice for all three actors at both budgets and both sliders (difference greater than1e-6s).
3. With common preparation opportunity, the best of Franz/Charles is faster than the best concentrated reference preparation at each budget (greater than1e-6s).

If2or3fails, retain the mixed/negative diagnosis; investigate the matched results before assuming stronger preparations solve the problem. Do not force the expected answer through trait changes. A positive pilot supports the era-I reachable-player audit and later joint nine-driver refit; it does **not** qualify V1, all circuits, an IA policy or physical global balance.

## Durable checkpointing

Keep the existing atomically written, checked native row per case. Append only a hashed row reference to a fsynced JSONL journal. Bind an immutable ordered matrix and executable/source identity once; reject row corruption, changed ordering or stale cache. Preserve then truncate only a trailing unterminated write, never discard malformed complete records. Resume existing checked rows without recomputation.

No full result-array rewrite/compression per GP. Export the complete compressed archive once after all journal entries validate, including every failure. Test interruption after native save/before journal commit, torn trailing line, idempotent resume, identity/row tampering and exact final archive reconstruction. Operational timings concern the harness; do not present them as browser/WASM speedups.

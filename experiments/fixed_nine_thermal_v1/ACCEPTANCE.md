# Fixed nine / thermal policy V1 — frozen native calibration

Follow-up to #457, #487–489. Freeze the selected rolling-heat-065 tire policy, all solver laws and the current qualified-profile roster (identical to retained roster V2). No trait increments, hidden pace factors, AI-only driving modes or runtime changes. Franz and Charles retain the strongest potential, without forced finishing positions. A player reference has .84/.84/.84 traits. All actors drive Balanced with stable identity-based random streams.

## Scope and budget contract

Era I: Monza, Silverstone, Zandvoort. Era II: Montreal, Spielberg, Mexico, Sao Paulo, Silverstone, Monza, Zandvoort. Full official GP lengths, initial fuel 100 kg, native serial execution, at most two build jobs. This task does not admit the candidate into the local browser, catalog, Authority, Verifier or deployment.

The input is the **available effective cap before the weekend**, including installed bonuses exactly once, never player spending, tuning, results or player pace. Same cap for all nine. Fixed-era rival equipment stays unchanged: I classic V8/no aero/floors 0000; II V8 1970/basic aero/floors 2141. The actual player keeps their fixture equipment and bonus floors; early era-II technology disadvantage is intentional. This is budget parity, not automatic hardware parity.

Qualified field ranges: I 8..12; II 12..24. Exact integer allocation and per-axis cap 20. Scale a measured preparation to anchor budgets I 8/10/12 and II 12/18/24; interpolate rational anchor targets, then deterministic largest-deficit projection respecting floors/caps. No silent out-of-range clamp/extrapolation. The resulting complete field is frozen before the weekend. Future integration must supply available cap and admit each supported range explicitly.

## Bounded measured proposals, no exhaustive optimization

For each of ten contexts take the fastest two **distinct physical inputs** among retained rolling-heat-065, Balanced, seed 73013, ordinary coupled-aero one/two-stop GPs at field-equivalent equipment. Use qualification plus the two explicit aero follow-ups. Exclude synthetic drag/CL controls, other seeds/modes, zero/three-stop diagnostics and older-physics performance. This is an imperfect sample, not a global optimum. Each proposal includes points, legal sliders and a complete service-inclusive stint schedule. All source/archive/input/result digests are retained in plan-v1.json.

Run both proposals for each named driver at I budgets 8 and 12 and II budget 24 (234 GPs). For each actor/context choose a single proposal using mean normalized endpoint time. Franz/Charles take their measured best; others choose by frozen style within 0.3% of their own best. Style uses stop count/timing, engine/chassis/cooling allocation and gearing, never a desired rank or player result. Run the chosen proposal at remaining anchors: I budget 10, II budgets 12 and 18 (153 GPs). Legal controls remain unchanged during scaling.

Player exit comparison: same known-baseline search allowance for informed and concentrated families, **five candidates each**, plus an equal-discretionary/default MH one-stop recipe (110 GPs). Baseline informed is fastest retained ordinary case, concentrated is fastest retained case with aero/cooling at installed floors. Four neighbors: gear ±.1 and I one-point chassis/engine transfers, or II downforce ±.1; fill invalid/duplicate neighbors with deterministic legal point transfers. Report sampled neighborhoods, not guaranteed optimality. Entry informed/routine player observations add 20 GPs. Actual fixtures, not fictitious starter upgrades, are used.

Resolve-only bridges for all 20 reference proposals must match retained physical-input digests; fresh full replays of first Monza proposal in each era must match old full-result digests (2 GPs). Fresh repeats for all nine selected exit preparations at Monza per era (18 GPs) must match input and full-result digests exactly. Training maximum **537 GPs**. Checkpoint every case, retain failures, cache by executable/source/policy/case identity and reject altered namespaces. Commit protocol before compute. Serial eras, no remote compute.

## Training gates and frozen validation

Every accepted GP finishes with exact lap-boundary continuity and pit preparation, charged pit service, elapsed-time closure and tire-energy closure at existing tolerances. Check effective cap/floors, policy, initial fuel, identity stream and window occupancy accounting. Any numerical failure or bridge/repeat mismatch blocks qualification.

At actual exit means per era: sampled informed player must win at least one context, median rank <=3, and beat routine time everywhere. The concentrated sample must not win every context. Preserve failures as REFINE, do not lower driver traits or alter physics to manufacture wins. Report aggregate driver ranks and Franz/Charles outcomes; another driver winning is allowed. Compare results over budgets, without expecting a monotonic race-time benefit in every constrained projection.

If training passes, freeze selected complete policy and player templates by hash before reserved runs. **66 GPs**, six groups of nine AI + informed + concentrated player:
- unseen interpolated cap I9 at Monza/Zandvoort, II16 at Silverstone/Mexico, seed 271828;
- exit caps I12 at Silverstone and II24 at Sao Paulo, seed 314159.

Reserved player templates scale the frozen exit choice to fixed field equipment/floors: explicitly controlled equal-equipment diagnostics, not actual entry-career snapshots. No new setup search, coefficient/trait tuning or selection after these results. Require all 66 numerical checks; report per-era player attainability and concentrated dominance as review gates (informed win somewhere in each era, concentrated does not win every reserved group). Failure leaves the candidate unqualified; a new protocol is needed to refine.

Maximum total **603 GPs**, plus inexpensive resolve-only probes. No old named-driver times are reused. Independent elapsed-car comparisons do not prove traffic interaction, real accidents or browser multi-car parity. Those, all-doctrine bindings, native/WASM equality, recovery, performance and versioned admission remain later gates. No exhaustive all-era/all-budget claim.

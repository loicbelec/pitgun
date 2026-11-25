# Fixed-nine budget calibration — REFINE

The bounded V1 campaign is complete, **not qualified for admission**. Do not confuse numerical correctness with gameplay acceptance, or training results with reserved results. Policy/source selection was frozen at `5d46019` before the 66 reserved GPs. The original 603-GP protocol and all thresholds remain unchanged; no retuning followed reserved results.

| Check | Result |
|---|---|
| Complete full GPs / accepted laps | 603 / 37,947 |
| Physical, boundary, service, budget, trait, fuel and tire-energy failures | 0 |
| Exact fresh named-driver repeats | 18 / 18 |
| Exact proposal physical-input bridges | 20 / 20 |
| Exact prior full-result replays | 2 / 2 |
| Actual career exit training, informed player | Era I: P1/P3/P3; era II: six P1 and one P3 |
| Concentrated training wins | Era I: 0/3; era II: 4/7 |
| Reserved informed player, era I | P3 Monza cap9; P2 Zandvoort cap9; P3 Silverstone cap12 — no win, FAIL |
| Reserved concentrated player, era I | P10/P10/P10 |
| Reserved informed player, era II | P1/P1/P1 |
| Reserved concentrated player, era II | P1/P1/P1 — sampled sweep, FAIL |
| Candidate decision | REFINE; no catalog, game, WASM or leaderboard admission |

## What was implemented

A separate native probe binds either actual career-fixture means/equipment or explicitly controlled fixed-era field means, the frozen three-compound rolling-heat-065 policy and the named driver's current V2 traits/mode/noise stream. A pure preparation helper scales effective point vectors to three anchors per era and interpolates integer available caps with exact sums, floors and axis limits. It has no input for player setup, spent points, results or pace.

All nine keep their identities, traits and Balanced driving mode. Rival hardware remains era-based: classic V8/no wing in I, V8 1970/basic aero in II. Available effective cap includes bonuses once; it is not the player's currently spent total. Nominal ranges are I8..12 and II12..24, with no silent extrapolation. The candidate policy is retained in policy-v1.json but marked unadmitted. Roster resources and solver equations were not changed.

Charles and Franz both average rank1.50 among the nine over the ten exit contexts, with Smooth next at3.10. Their potential emerges without a forced result or hidden pace multiplier. This is consistent with the desired identity, but does not override the failed gameplay gates.

## What the two failures do and do not establish

**Era I:** the five-candidate prepared-player neighborhood did not demonstrate a reserved win. Its deficits to the winner are approximately6.25s at Monza,7.59s at Zandvoort and7.69s at Silverstone over complete GPs. This does not prove the field unbeatable: that neighborhood varied gearing and chassis/engine transfers, but did not systematically explore cooling transfers, all legal gearing or pit timing. The next check should expand legal player preparation before weakening rival traits.

**Era II:** the concentrated reference has installed aero2/cooling1 and an upgraded engine. It is not a cooling-zero starter. It is competitive in four of seven training contexts and sweeps the three reserved groups. This violates the preregistered non-sweep gate. The field was drawn from two retained joint proposals, while the prepared player also received four local adjustments. Better coupled rival preparation is a plausible correction, to be measured; this result does not prove cooling has no physical effect or justify changing coefficients now.

No general human win rate, global setup optimum, all-budget fairness, traffic interaction or broad incident-risk calibration follows from these samples. Later eras, invitations and Monaco remain outside this campaign. Reference player traits are .84/.84/.84. Initial fuel is100kg throughout. Current browser preview :5252 and deployed services remain unchanged.

## Reserved-result clarification: beating the field is not setup dominance

The concentrated player beats the nine rivals in all three reserved era-II groups, but the prepared alternative is faster in all three **against the same frozen field**:

| Circuit / effective cap | Alternative faster than concentrated by |
|---|---:|
| Silverstone / 16 | 0.705 s |
| Mexico / 16 | 4.535 s |
| Sao Paulo / 24 | 14.495 s |

Mexico supplies the closest controlled point-allocation comparison: same reference driver, seed271828, fuel, installed equipment, gear0.5, downforce0.4 and one-stop medium/hard plan at lap35. Effective A/C/E/K changes from **[2,6,7,1] to [2,5,6,3]**, keeping cap16. The extra cooling trades one chassis and one engine point for two cooling points. Minimum engine thermal power factor is0.942 versus1.000; the concentrated preparation is thermally limited during76.9% of driving time versus0% for the alternative. The alternative finishes4.535s sooner. This tests a legal allocation tradeoff, not the isolated derivative of cooling at constant engine/chassis.

Silverstone and Sao Paulo also change the stint schedule, so their gains cannot be attributed to cooling alone. These observations do not rescue the failed non-sweep gate or establish universal physical balance. They show that the sweep is not evidence that motor/chassis is the best physical preparation everywhere. Rival preparation is the first hypothesis to test: rivals were selected from two retained joint proposals; each player family also received four local adjustments. The successor comparison should give rival preparation a comparable coupled setup/strategy opportunity, retaining fixed budgets/traits and genuinely new validation cases.

## Next bounded iteration

1. Preserve V1 as negative qualification evidence; never relabel its reserved cases as fresh validation. Keep the physical policy and roster frozen.
2. Era I first: a separately committed attainable-setup audit with additional legal cooling/point transfers, gearing and pit-timing controls. Give the concentrated comparison an explicit, comparable search allowance; distinguish zero-discretionary-cooling from wider engine/chassis-heavy allocations. Determine whether preparation evidence is too narrow before changing the field.
3. Era II separately: add ordinary joint setup/stint proposals from the offline reference studies, including local slider changes. Actually measure the named drivers, keep style-based imperfect choices, and check low/mid/high caps. This uses authored offline research, never live player settings/results. Do not add artificial pace taxes or force spend on all four axes.
4. Count and commit the finite matrices/criteria before compute; reuse native cases only with identical physical input/executable/policy bindings and label reuse honestly. Keep one worker and serial eras. Fix any checkpoint cost in the orchestration before the next long batch, without altering physical steps.
5. Freeze the revised policy/templates, then use entirely new reserved budgets/seeds. Only a passing numerical and gameplay review supports the native/WASM/recovery/performance and versioned local-game admission steps.

See RESULTS_V1.md and the compressed per-lap archives for the full tables and provenance. The selected model's physical qualification remains distinct from this failed opponent qualification. #457/#490 stay open; the broader point-value issue #469 and selective verification #480 keep their independent scopes.

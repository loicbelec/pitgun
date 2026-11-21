# Competitive opponents on the new physics — 22 November 2026

**Protocol and resolution audit, not a fitted policy or balance verdict.**
The event journal/axis is instrumentation. No event/display work changes an AI
allocation, setup, driver, strategy or physical result. Further UI/runtime work
is secondary to the driver/physics and competition work specified here.

## What the code actually does

There are three independent concerns: the physical driver, the opponent policy
(car budget/setup/strategy), and the Engineer controller. The event journal
records committed facts; it does not select any of their coefficients.

| Mechanism | Existing implementation | New thermal candidate / current limitation |
| --- | --- | --- |
| Legacy aggressiveness | `Driver.aggressiveness`,0–1. V9 resolves cornering `.94+.05a`, braking `.93+.06a`, traction `.95+.04a`, error `.04-.03a`. | Larger `a` increases utilization **and reduces** small control error. This is not the intended performance/incident-risk tradeoff. It remains an explicit physical input even without a player slider. |
| Limit exploitation | V2 driver trait; with mode commitment, sets useful force utilization. | Separate resolver exists. Its catalog presence does not enable it in career V9/thermal V2. |
| Consistency | V2 driver trait; reduces deterministic utilization-error amplitude. | These are small reductions of useful utilization, not spins, collisions or accident probabilities. |
| Tire management | V2 driver trait; reduces the historical correction-workload multiplier. V11 can reserve tire-force capacity for correction demand. | Thermal V2 explicitly rejects the old extra correction-heat proxy. The trait needs a physically accounted connection to the new contact/thermal model, not a silent wear discount. |
| Manage / balanced / attack | Separate requested commitment, catalog values `.6/.8/1`, distinct from persistent ability. | Energy V1 accepts a fixed envelope and rejects a changing driver schedule. No mid-race aggression command is admitted by this candidate. |
| Legacy tire/noise/pace modifiers | Compatibility kernel has derived `DriverEffects`. | Do not ascribe these to the native V3 kernel: it uses physical controls, not that compatibility lap-time bonus path. |
| Opponent strategy “aggression” | An older game heuristic uses a style-derived value to choose compounds/stops. | This is not `Driver.aggressiveness`. Governed policy V2 instead selects two one-stop profiles, medium→soft, near55%/82% distance. It is not an incident model. |
| Incidents / VSC | No actual driving-error/accident model in the inspected Solver/Simulator. | Future work. A chart marker, random lap-time addition or LLM sentence is not a simulated incident. |

V2 catalog archetypes are balanced reference, limit specialist, smooth operator
and tire manager. Each has the same2.52 sum of trait values (balanced `.84` each;
specialists `.96/.78/.78`). Equal sum does not prove equal racing strength.
The player is assigned `balanced_reference` by the governed game preparation,
without a UI choice; other paths use `default`. Record the actual resolved
profile and requested mode for both player and rivals.

### Resolution gate before fitting

`preparePitWallSession` assigns the four V2 IDs when an opponent policy is present.
The inspected thermal career controller starts V9 with no V2 driver experiment.
V9 uses the legacy driver lookup, which falls back to `default` for missing IDs.
`balanced_reference`, `limit_specialist` and `tire_manager` are in `drivers-v2`,
not the legacy map. `smooth_operator` exists in both, with distinct meanings.
Consequently an apparent archetype selection can resolve to default controls.
This was already identified in the earlier one-car thermal audit; it must become
an explicit admission/calibration blocker. Do not change old catalog/runtime bytes
or fallback semantics silently: select/validate the proper schema in a new version.

The native audit below captures requested/resolved identity and controls, plus
the **separate** V2 arithmetic. It does not execute a race, assert current VPS
behavior or prove that V2 traits are connected to the new thermal solver.

```sh
cargo build --release --locked --offline -p pitgun-racing-simulator \
  --example driver_resolution_audit
target/release/examples/driver_resolution_audit "$PWD"
```

Relevant source: `racing-contract/src/driver.rs`, Simulator `resolve_driver`,
`resolve_v3_driver_control*`, `control_prototype.rs`; Solver native validation and
`resolved_driver_utilization_profile`. Game: `pitwallSessionExecution.ts`,
`competitors.ts`, `opponentPolicy.ts`. Exact repository prefixes and hashes are
recorded in the audit/campaign files.

## Delivery order

1. **Effective driver contract.** Resolve the intended driver schema explicitly;
   player and rivals share the same laws. Keep a fixed balanced player mode until
   a UI control is deliberately introduced. Separate ability from requested risk.
   Test one trait at a time with unchanged noise identity; changing a named driver
   also changes the existing seeded utilization stream, so it is not an ablation.
2. **Coherent performance and risk.** Reuse the richer utilization/consistency
   structure, reconnect tire management through accounted contact losses/control
   decisions, and introduce a versioned reduced mistake/incident model. Increasing
   commitment must raise demand and modeled risk, modulated by ability/consistency
   and the actual grip state. No global time handicap or hidden cooling-zero tax.
3. **Physical sensitivity and legal horizons.** Screen allocations/sliders and
   driver responses with native one-car runs, then legal stints/full GP distance.
   Fuel sizing/exhaustion#445 and the conservative next-lap grip seam remain known
   blockers; no150kg reserve or removal of failed finishes to make a policy look good.
4. **Opponent fitting.** Once those mechanisms/profile are frozen, select coherent
   budgets, circuit setups, driver/commitment roles and multiple legal strategies.
   Reuse existing corpus/policy builders instead of moving numerical setup laws
   into Python. The old135-race corpus is useful scaffolding, not evidence that
   the new thermal model is balanced.
5. **Reserved evaluation and admission.** Freeze policy before evaluating reserved
   seeds/conditions, all era calendars and legal player configurations. Then select
   an explicit runtime/catalog/observation version. Only then resume actual
   Worker/WASM/Pit Wall activation. Event relevance/Engineer triggers follow real
   competition; do not tune opponents merely to produce a quota of decision bubbles.

## Campaign design

[plan-v1.json](plan-v1.json) records the staged protocol and blockers. It is not
an executable run corpus until its identity/fuel/driver/incident gates are met.
All calculation is local. Start with a cost pilot and4 native workers; shortlist
before full-field/full-distance runs. No Databricks or provider calls required.

Use anonymous, authored career-state fixtures from real reducers/preparation:
era entry/exit, earned point budget, actual HQ offsets/bonuses, components,
doctrine, unlocked sliders/compounds and legal calendar distances. Synthetic
8/15/20 budgets alone do not establish playability. Distinguish matched-component
experiments from deliberate opponent technology advantages in real progression.
Cover all public eras, prioritizing I/II. EraI regular tracks are Monza,
Silverstone and Zandvoort; use `seasonContent`'s introductions/full calendars for
later eras. Monaco remains explicitly owner-deferred, never silently counted PASS.

The paired baselines are:

- the best sampled **motor/chassis-only spend**, with both the split and legal
  sliders optimized; record HQ bonuses separately (zero spend is not zero physics);
- near-concentrated spends (>=80% motor/chassis), balanced and circuit-informed;
- all feasible integer allocations for small budgets, then a bounded deterministic
  space-filling/local-transfer search for larger budgets;
- isolated allocation comparisons, isolated slider comparisons, isolated tire/pit
  comparisons, then combined setups. Never infer an allocation effect from a
  comparator which also changes the strategy/driver.

Test player-legal slider endpoints even if the current authored AI policy only
searches `.15–.85`. Honor locked settings. A setup change must not weaken/reseed
the same opponent field or reveal the player's future plan. Keep the9 rival
contracts fixed per paired scenario, including their components, initial modes,
budget and independent random-stream identities.

Report paired pace/finish gaps, ranks/wins, season performance, parameter regret
and sampled best responses; also derating duration/actual shaft work, tire surface/
carcass/grip/wear, fuel, faults/DNFs when implemented and uncertainty across seeds.
Three fitting seeds are a debug screen, not a rare-accident probability estimate.
Use a larger independent seed set for the shortlisted stochastic evaluation;
report denominators/intervals and all failures rather than the surviving winner.

## Competitive acceptance

The owner's target is thoughtful point/slider choice rather than routine wins by
motor/chassis concentration. Do not turn this into an arbitrary obligation to
spend on all four axes. An engine-oriented setup can be legitimate at Monza, and
an incident can occasionally hand a poor setup a win. The rejection is a robust,
repeatable winning shortcut across the season or clearly inappropriate circuits.

Proposed checks, frozen before a full verdict:

- REJECT a policy if the optimized concentrated baseline routinely wins the
  entire EraI calendar across reserved seeds without meaningful setup/strategy
  adaptation. Show the per-circuit and season results, not just an average.
- On technical/thermal validation cases, a feasible allocation outside that
  family must yield a reproducible strict gain above numerical uncertainty, and
  the field must exploit that opportunity. If physics supplies no such response,
  repair the physical tradeoff before fitting a stronger opponent multiplier.
- Informed decisions must improve outcomes against the **same** field, while
  attainable legal configurations can beat it. Reject an impossible field and
  a universally dominant driver/maximum-commitment mode.
- Preserve the owner's accepted modest early five-lap cooling effect. Do not
  inflate derating solely to force cooling points or make the UI more dramatic.
- Use incident-disabled paired diagnostics to separate setup/pace from luck,
  then evaluate expected outcomes and failure tails with incidents enabled.

Win-rate/difficulty bands require an explicit design decision before final fitting;
none is retroactively inferred from whichever coefficient set wins this screen.
No balance ACCEPT, global optimum or real-world accident fidelity is claimed here.

## Deterministic incident requirements

First isolate a reduced single-car mistake with actual future state consequences
(trajectory/speed, elapsed time, heat/fuel/wear or explicit retirement). Define
units/exposure and a stable seeded stream, distinct from ordinary pace noise.
No draw per render frame, telemetry sample, retry or LLM call. Integrate risk over
declared simulation exposure so changing exportHz does not change probability.
Persist exposure/draw continuity and prove continuous/chunked/reloaded parity.

Facts enter the native journal only when committed. Weather, collision and VSC
are separate capabilities; VSC requires a shared legal frontier for all affected
cars. Overlaying a banner on prefetched independent futures cannot implement it.
Risk coefficients are a reduced game hypothesis until supported by suitable
references/data, not an industrial safety prediction.

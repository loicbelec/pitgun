# Era-I attainable preparation — frozen development audit

Follow-up to #490/#457 and the equal-preparation Mexico pilot #491. Preserve V1 REFINE, its opened reserves and the mixed Mexico diagnostic. This is the next approved preparation audit, not a new physics fit, runtime admission or fresh holdout evaluation. No game, catalog, hosted service, roster trait, driving mode, coefficient or sampling change.

## Matrix and resource ceiling

Three regular circuits: Monza53, Silverstone52, Zandvoort72. Effective caps8/10/12 include installed bonuses exactly once. Cap8 uses the actual era-I entry fixture; cap12 uses the actual exit fixture with chassis1 floor. Cap10 is an explicitly controlled equal-equipment intermediate case with floors0000, not an invented authored career snapshot. Reference driver .84/.84/.84, Balanced, seed73013, initial fuel100kg, classic engine/no adjustable wing; downforce stays0.5.

Three preparation families, six authored joint proposals each, for all nine circuit/budget contexts:
- `zero_extra`: no discretionary points in aero/cooling, respecting fixture floors;
- `heavy_80`: at least80% of **discretionary** points in chassis/engine; some cooling is allowed;
- `free`: no family restriction on the legal point allocation, with a bounded sample, not global optimization.

Entry baselines use retained ordinary selected-policy studies; cap12 uses the best retained V1 reference templates; cap10 projects those exit templates to the controlled cap. The heavy baseline allocates the maximum integer20% discretionary allowance to cooling and shares the remaining points over chassis/engine according to the free baseline ratio. Families can overlap; do not pretend their samples are independent populations.

After baseline, fixed candidate slots are: gear0.15; pit3laps earlier; cooling->engine with gear0.15; engine->aero with gear0.15; cooling->chassis with gear0.15. If a slot is illegal or duplicates an earlier slot, use its explicit ordered fallback list in plan-v1.json (chassis/engine transfers with gear0.15, gear0.05, later pit). Each group retains exactly six legal distinct proposals. Stop count/compounds come from the recorded ordinary one-stop soft/medium baseline; pit shifts preserve full distance and service cost. No unavailable downforce changes or unrecorded local optimization.

Total162logical evaluations. Deduplicate only **entire typed native cases equal except Case.id**: the frozen Rust probe never consumes Case.id in RunRaceRequest/solver input. Retain every logical family membership and its canonical case reference, with assertions/tests that no physical field differs. This yields147unique primary solves, not162independent runs. Add six predetermined fresh repeats: zero-extra and free baseline at cap12 on each circuit. Total **153new full GPs maximum**, within the approved168ceiling. No old player times imported as fresh solves.

One native process. Default invocation cap20new GP or300seconds, checked between cases; finish/persist the active GP, then exit without automatic next batch. First delivery launches only one such batch. Source/matrix identity stays frozen across later resumptions; changed scheduling limits do not change physical inputs.

## Immutable rival field reuse

The nine rival preparations, equipment, traits, seed and streams are exactly those in the retained V1 policy at caps8/10/12. They do not respond to player tuning or results. Reuse81already-solved rival GPs (nine actors x nine contexts), clearly tagged reused, from the hashed V1 training archive and same native executable. Require exact typed case equality excluding non-physical id, existing numerical checks, and a native resolve-only bridge to the recorded full physical-input digest for every reused case. Retain source case/input/result identities and elapsed time. Reuse is not a fresh solve or repeat. A failed bridge blocks player compute.

Actual player fixtures and controlled cases retain their own legal floors/components. No extra bonus addition, entry engine upgrade, fictitious fuel, elite trait boost, forced podium or pace multiplier. Commit all protocol/generator/checker inputs before compute. Native binary SHA remains the V1/pilot c5443499... identity from plan-v1.json.

## Gates and interpretation fixed before compute

All accepted player solves must pass the existing complete-finish, lap-carry, pit-reset, service/elapsed accounting, tire-energy, cap/floor/fuel/policy checks. Six fresh repeats must exactly match input and full-result digests. All81reused rival cases must bridge. Retain numerical failures; never silently remove a failed case or certify a partial batch.

Only after all153jobs complete, report the best service-inclusive time, frozen-field rank, points/gear/stints and gap for each family/context. Development review supports proceeding if:
- no numerical/replay/field-binding failure;
- the sampled free player wins at least one actual exit context and its median exit rank is at most3;
- the zero-extra family does not win all nine contexts.

Report heavy80outcomes without treating the concentration percentage itself as failure: a legitimate setup with some cooling may remain heavily motor/chassis-oriented. Do not require arbitrary spending on every axis or that more cooling always wins. Report actual limitation/temperature/wear to explain choices. The small candidate pool, observed ranks and this single opened seed do not establish a general human win rate, global attainability envelope or all-axis causal sensitivity. If review fails, retain REFINE and diagnose preparation coverage before changing driver ability.

This development audit leaves V1's failed reserved results unchanged. It does not qualify later eras or a new opponent policy. Era-II common preparation refit, fresh reserved budgets/seeds after complete policy freeze, native/WASM/recovery/performance and versioned local admission remain later gates. CoverageIII–V stays in #492. Keep one clear owner-facing preview; this batch does not update it.

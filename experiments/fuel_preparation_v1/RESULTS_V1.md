# Fuel preparation/outcome V1 — local result

**Outcome isolation qualified natively; automatic sizing remains REFINE.**
Core/screen source994d6eb; ceiling diagnostics82dbf21. No physics coefficient,
consumption, radiator, driver risk or tank size was fitted. These are new synthetic
native cases, not the exact missing-seed historical observation in issue445.

## What now works

- A typed fuel shortfall is distinguished from technical errors without parsing
  strings. The opt-in native adapter returns the last fully committed lap/service
  boundary and failed budget. It does not claim an exact within-lap stopping point.
- A fuel-limited rival can terminate while a healthy player finishes with the
  exact standalone result digest. Technical failures remain errors.
- Initial preparation uses two pilot laps per planned compound, at most three
  compounds. The tested V8 full-GP plans use four pilot laps, never the whole GP.
  Policy, initial/pilot/prepared inputs and outcomes are recorded separately.
- The12-case screen repeats exactly twice. Ten estimates produce a prepared run;
  all ten finish above the1kg reserve. Two modern estimates exceed the110kg test
  limit and return no prepared request. All original-load failures are retained.
- The56 previous driver-case inputs/outcomes match exactly, including five frozen
  thermal baselines and three additional unchanged-service comparisons.22 older
  historical/energy outcomes also match, including their two expected fuel errors.
- 268 native tests pass (Authority30, Simulator135, Solver25+56, Verifier16+6).
  Scoped strict Clippy, all-target workspace check, formatting and diff pass.

## Preparation screen

Same fixed policy throughout:110kg operational test ceiling,1kg planning reserve,
8% provisional uncertainty and upward0.5kg rounding. The110kg value is **not** a
universal F1 rule or an identified physical tank capacity.

| Scenario | Mode | Prepared start kg | Actual finish kg |
| --- | --- | ---: | ---: |
| Early V8,Monza5,cooling0 | Balanced |4.0 |1.542 |
| Early V8,Monza5,cooling2 | Balanced |4.0 |1.528 |
| Classic V8,Monza53,cooling0 | Manage / Balanced / Attack |85.0 /85.5 /85.0 |11.647 /12.211 /11.947 |
| Classic V8,Monza53,cooling2 | Manage / Balanced / Attack |86.0 /86.5 /86.0 |9.975 /10.583 /10.310 |
| Modern,Barcelona20,medium | Balanced / Attack |43.0 /43.0 |10.219 /10.424 |

The excess reserves show conservatism, not proof of optimal preparation. These
are initial-plan estimates; future weather/strategy/driver changes are not bounded
by this screen. No player fuel slider or current game behavior was changed.

## A rejected estimate is not physical infeasibility

The modern Monza53 estimates are132.5kg (cooling0) and141kg (cooling2), above the
unchanged110kg test ceiling. A separate diagnostic plan was frozen and executed
twice at exactly110kg, without applying the estimate:

| Modern Monza53 | Original100kg | Diagnostic110kg | Reserve target |
| --- | --- | --- | --- |
| Cooling0 | Fuel shortfall attempting lap50;49 fully committed | Finishes with4.087kg | Met |
| Cooling2 | Fuel shortfall attempting lap49;48 fully committed | Finishes with0.885kg | **Not met** |

Both ceiling outcomes repeat exactly. Thus the two-lap maximum-rate estimator
can reject a physically completable race. It intentionally over-extrapolates
fresh/heavy pilot demand across the entire distance. Later fuel mass, worn tires,
temperature and pace change actual work. It cannot establish a minimum required
load or justify a larger tank, altered BSFC or an invented hybrid contribution.

Consequently its earlier outcome name `Infeasible` is superseded in the native
API by **`EstimateExceedsLimit`**, with `estimated_start_kg` and `maximum_start_kg`.
The initial screen is retained verbatim, including its historical label; it is
not rewritten to appear to have made the correct physical diagnosis. Only this
label/field clarification changed after measurement, not the estimator equations,
policy, Solver, inputs or numerical results. Focused preparation tests and strict
Clippy are rerun after the clarification.

The1kg value is a declared planning target in this experiment. The0.885kg result
does not invent a regulatory disqualification. Automatic preparation is not
accepted for every modern configuration: improve/qualify the estimate and reserve
on a broader reserved corpus before admission. A knowingly capped estimate must
not be silently presented as a guarantee of finishing.

## Real career inputs ready for the next screen

[The exporter](../opponent_physics_v2/export_career_fixtures.mjs) runs actual game
reducers and session preparation without reading a player's save/browser.12
entry/exit snapshots across the three doctrines yield54 full-GP requests with
nine rivals each. Both exports match exactly, and changing the player's allocation
to all-engine leaves the rivals byte-identical.

This authored route yields8→11→16 discretionary points plus actual HQ offsets.
EraII entry may still have the old engine; the installed engine/aero must follow
purchases, not simply the era or baseline vehicle ID. The snapshots use explicitly
synthetic two-lap P9 accounting observations to earn announced rewards. They prove
input generation, not physical pace, wins, difficulty or real play duration.

## Evidence and remaining work

`results/discovery-v1.json`: full initial fuel screen/driver regression;
`results/capacity-diagnostics-v1.json`: repeated ceiling diagnostic;
`results/historical-regression-v1.json`:22 older frozen outcomes.
The aggregate qualification receipt binds their file hashes and source identities.

Next are boundary look-ahead/numerical-cost qualification (#446/#420/#421), fuel
prediction/reserve and precise domain stop/shared-journal semantics (#445), then
real-career allocation/slider/tire screening and fixed-field opponent fitting
(#457). The current game is unchanged; there is no new signed native/WASM/export,
process-checkpoint or staging/production admission. Do not declare the optimized
engine/chassis shortcut solved from these one-car preparation cases.

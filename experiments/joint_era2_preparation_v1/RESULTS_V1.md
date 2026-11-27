# Joint era-II preparation — development PASS

Source/protocol freeze `a3d11f7`; all [preregistered gates](ACCEPTANCE.md) pass.
This fits opponent preparation and compares the player in **one campaign**, not
a second player-only audit against an unchanged field. Driver traits, mode/noise,
physical model, tire policy, fuel and equipment did not change.

## Evidence and accounting

- Seven circuits, three effective caps, ten actors and four common preparations:
 840primary rows plus18fixed fresh repeats,858total /56,316laps including reuse.
- 279original V1 rows reused unchanged after840current native input resolutions,
  with84cross-actor common-equipment comparisons. Original case/output echoes and
  source row/archive/input/result hashes are retained.
- **579new full GP /37,987new laps**; zero numerical failures,18/18exact fresh
  full-result repeats. New native calls sum2,434.35s (40.57min), average4.20s/GP;
  excludes resolution, hashing, checkpoint and reporting time. Reused wall times
  are not included as new computation.
- One worker, one entire era invocation. The first13native files/mtimes and journal
  prefix stayed unchanged. Completed-cache resume adds0new GP and preserves all
 1,700native JSON files/mtimes plus journal/archive/decision/policy bytes.
- Decision and policy regenerate exactly from retained evidence.189complete
  setup/stint anchors are frozen.819structural integer-cap projections checked;
  this does **not** qualify their unmeasured non-anchor native performance.

## Player opportunity and shortcut resistance

The player result is the fastest reference among the same four preparations
available to each rival. It represents preparation opportunity, not a measured
distribution of player skill or an automatic setup recommendation in the game.

| Effective cap | Vehicle scope | Prepared wins /7 | Median prepared rank | Zero-extra wins /7 | Circuits with strictly better unrestricted time /7 |
|---|---|---:|---:|---:|---:|
|12|Controlled common upgraded car|5|1|2|7|
|18|Controlled common upgraded car|4|1|2|6|
|24|Actual craft-engineering career exit|4|1|2|5|

`Zero-extra` means no discretionary aero/cooling points **above installed floor
[2,1,4,1]**, not a physically uncooled engine. This shortcut may legitimately win
some circuits, but does not sweep the era at any tested cap. A wider heavy80
engine/chassis allocation remains reported separately in the decision artifact.

At actual exit cap24:

| Circuit | Prepared rank | Zero-extra rank | Unrestricted gain over zero-extra (s) |
|---|---:|---:|---:|
|Montréal|1|2|9.790|
|Spielberg|2|2|0.172|
|Mexico|1|1|0.000|
|São Paulo|1|3|14.367|
|Silverstone|1|1|0.000|
|Monza|2|3|1.603|
|Zandvoort|2|3|14.625|

Mexico and Silverstone legitimately favor the tested concentrated preparation at
this cap. Other tracks reward different allocation/aero/stint compromises. The
result supports contextual choices, not a requirement to maximize every axis.
Lower-cap12/18 reference cars have upgraded common equipment; they are explicitly
not actual early-career vehicle snapshots or a promise of early-career wins.

## Fixed nine

All nine use their own measured times and frozen styles, within0.3% of their own
sampled optimum. No player's result enters selection. The fixed midpoint tie
preference preserves imperfect preparations; measured regret is retained per
anchor and reaches22.169s. This is a deliberate style allowance, not proof the
field has optimal setups. There is no forced finishing order or AI-only boost.

| Driver | Mean rank among nine at cap12 | At18 | At24 |
|---|---:|---:|---:|
|Franz Hermann|1.71|1.57|1.57|
|Charles L’Éclair|1.43|1.57|1.57|
|Smooth Operator|2.86|3.29|3.14|
|Luis Amilton|4.14|4.57|4.29|
|Isa Kadjar|5.29|4.57|5.14|
|Battery Voltas|6.43|6.57|6.29|
|Pedro Gaseoso|6.43|7.00|6.86|
|Daniel Enchantier|7.71|6.86|7.14|
|Goat Tifi|9.00|9.00|9.00|

Franz and Charles have the strongest average field results without changing their
characteristics. Ordering can change by circuit/budget. Each driver now has full
point/gear/aero/stint anchors per circuit and cap, not a universally fixed recipe.

## Boundaries and next gate

This is bounded development evidence on opened seed73013. It is not new reserved
validation, a global optimization result, all-doctrine transition validation,
native/WASM parity or runtime admission. V1's failed reserved results remain
unchanged and opened. Freeze a consolidated era-I/II candidate before new seeds
and budgets, then qualify native/WASM/recovery/performance and deliver one clear
local I/II preview. III, IV and V follow as separate whole-era joint lots.

No game preview, catalog/model identity, staging or production service changes
here. The local intermediate preview has not adopted this field.

Retained outputs: `results/era2-v1.json.gz`, `decision-v1.json`, `policy-v1.json`
and `execution-observations-v1.json`. The native archive SHA256 is
`9d54841ed2d97e0c0dcb5fa58e02e8fe31e81bf9ac811b3fb126cc3953017d45`.

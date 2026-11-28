# Era-III pilot — REFINE before field fitting

Source/protocol frozen at21717df, native SHA477f0d72d8b1a0399d81cf1917538b9990a09523297d30970cfd7f8572d2dc38.
Selected rolling-heat-065 and rosterV2 unchanged. No I/II runtime/game changes.

## Result

22 attempted single-car GP,1,315 fully accepted laps,65.50seconds total native
calls, one process. This is **not22completed GP**: two actual-entry V8 cases
finish all66laps; all20turbo attempts (including the one fresh repeat) terminate
with an explicit fuel shortfall. No fixed-nine policy is selected or qualified.

| Group | Cases | Outcome with fixed100kg |
|---|---:|---|
| V6T, cap34, three actors × three recipes × one/two stops |18|None finish; last complete lap56–62|
| Actual V8_1970 entry, cap24, two reference recipes |2|Both finish;22.15/23.41kg remain|
| V6T, available82/spent80 saturation smoke |1|Last complete lap57|
| Fresh repeat of Franz neutral one-stop |1|Exact same partial outcome and retained lap data|

The entire cached case data and partial/complete outcomes reproduce for the fresh
repeat. Completed-journal resume computes0new GP and preserves hashes and mtimes
of native files, manifest, journal and final archive. The original native binary
has also been rebuilt from the frozen source and reproduced exactly, and is kept
privately beside the later diagnostic executable.

All99exported doctrine/circuit component inputs resolve correctly. Six focused
native tests (five initial gates plus the later fuel-receipt bridge), seven Python
tests and14shared journal/batch tests cover the source boundaries and durable
runner. The I/II game suite was not rerun for this offline-only slice because no
game code was changed. The5253preview remains available for owner I/II testing.

## Interpretation and next boundary

The pilot's fixed100kg came from the retained I/II comparison scope. It is neither
a universal F1 rule nor evidence of a V6T tank capacity. Current turbo cases cannot
be used as valid finish-time training rows. Do not tune talents, tire coefficients,
seeds or pit-loss timing to conceal this failure. Failed prefixes remain in the
archive rather than receiving a synthetic race time or refuelling event.

Native automatic initial-fuel preparation already exists under issue445. The next
bounded diagnostic reuses its unchanged110kg operational-limit/1kgreserve/8percent
uncertainty policy on four original resolved inputs, plus one exact repeat. Inputs
must bridge the pilot fingerprints exactly. Preparation computes at most two laps
per planned compound; no full GP is precomputed. Its estimate is not tank-capacity
identification or proof of physical impossibility if it exceeds a limit.

The accepted prefixes also reach peak tire surface temperatures about128–152°C.
These are surface-state extrema, not average temperature, and the pilot does not
qualify thermal balance for all IIIcircuits. Review window exposure/degradation
with complete fuel-feasible GP before drawing strategy conclusions.

Operationally, the pilot completes in about one minute on this Mac; a broader
joint matrix can be sized from measured single-car costs only after fuel handling
is explicitly resolved. III/IV/V remain serial lots. No public catalog, signed
workload, provider or deployment is admitted by these diagnostics.

## Audit artifacts

- `results/pilot-v1.json.gz`: original complete22-case archive, including failures.
- `results/pilot-review-v1.json`: compact failure-preserving review and resume proof.
- `pilot-plan-v1.json` and `ACCEPTANCE.md`: original frozen design.
- `fuel-diagnostic-plan-v1.json`: separate post-pilot diagnostic, not new holdout data.

The first console summary assumed every outcome carried a successful result
digest; it raised only **after** all22rows and the final archive were durably saved.
The reporting helper now handles partial outcomes explicitly and has a regression
test. No GP was recomputed to repair reporting; the raw frozen archive is unchanged.

## Existing automatic-fuel diagnostic — c4f67e2

Four original resolved physical inputs bridge **exactly** to the100kg pilot.
The unchanged110kg-limit preparation policy makes bounded two-lap-per-compound
estimates. No full GP is computed in this diagnostic.

| Input | Estimated starting mass, including existing policy reserve/margin | Existing110kg-limit outcome |
|---|---:|---|
| Franz, V6T cap34 neutral one-stop |133.5kg|EstimateExceedsLimit|
| Reference, V6T cap34 neutral one-stop |134.0kg|EstimateExceedsLimit|
| Reference, actual V8 entry cap24 |89.5kg|Prepared input available|
| Reference, V6T available82/spent80 two-stop |138.0kg|EstimateExceedsLimit|

One exact repeat of the Franz estimate,20pilot laps including that repeat. Source
input is never mutated; receipt binds the original input, preparation policy,
pilot inputs and optional prepared input. Completed-cache resume performs no new
solver calls and preserves every native receipt/result byte and mtime.

The limit remains an **experimental operational bound**, not a historical tank or
F1 rule. These estimates are not measured minimum fuel requirements. Prior work
already demonstrated conservative estimates exceeding a limit even when an actual
GP could finish at that limit. Neither raising the limit blindly nor reducing
consumption to make it pass follows from this diagnostic.

Next under#445/#496: compare bounded actual110kg outcomes with estimates on a
small fixed turbo set, then select/document an era/vehicle-aware automatic initial
fuel policy and validate finish reserve. Keep capacity, initial charge and reserve
separate. Only then freeze the full joint era-III field matrix. I/II playtesting
continues on its unchanged100kgqualified candidate; no player fuel slider is added.

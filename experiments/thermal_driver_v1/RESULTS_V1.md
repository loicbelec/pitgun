# Thermal driver V1 — qualification result

Source `8c289df`, policy and plan frozen in `f3187e8`, catalog1.8.1.
**Mechanism screen qualified; competitive calibration remains REFINE.**
No runtime/catalog admission or current game activation.

The final [machine receipt](results/qualification-v1.json) binds compiled sources,
binary, policy, resource, initial input and complete result identities. Its
[checksum](results/qualification-v1.sha256) covers the committed receipt.

## Reproducibility and regressions

- 56 cases, two exact whole passes:51 opt-in driver cases plus five frozen V2
  baseline cases. All complete. The3 full-GP cases with explicit service
  comparison also produce identical complete results, in both passes.
- 22 one-car historical/energy outcomes match their frozen input/result digests
  or errors. This includes the **two expected Barcelona66 fuel failures**, which
  remain failures; it is not a claim that those fuel cases now finish.
- 140 native simulation executions in the final bank:112 normal,6 additional
  service-continuation,22 historical. No full telemetry archive is committed.
- 263 native tests pass: Authority30, Simulator132, Solver25+54, Verifier16+6.
  Strict scoped Clippy for modified libraries/new probe, all-target workspace
  check, formatting and diff checks pass. No browser/WASM qualification claimed.

On the56 final outputs, maximum segment vehicle-work residual is
`3.64e-12 J`; maximum held-grip raw contact utilization including the correction
reservation is about`.999813`. These are numerical accounting checks, not a
validation of the physical coefficients against measured racing data.

The initial `f3187e8` screen had14 rejected trajectories: scaling the longitudinal
remainder alone overbooked the shared contact circle at high lateral demand.
Its failures and repeat/regression identities are retained in
[rejected-f3187e8.json](results/rejected-f3187e8.json). `8c289df` reserves capacity
inside the combined circle; policy, cases and acceptance tolerances are unchanged.
The two service comparisons that could be reached initially were equal; the
Attack comparison was not reached because its normal trajectory failed.

## Actual mode tradeoff

Balanced-reference abilities `.84/.84/.84`, common actor noise, seed42.
These are **two different frozen scenarios**, not an isolated circuit derivative:
classic15-point Monza5laps versus modern Barcelona20laps on one medium stint.
Within each scenario only requested mode changes.

| Mode | Error amplitude | Hesitation hazard /100km | Monza5 time s / final wear | Barcelona20 time s / final wear |
| --- | ---: | ---: | --- | --- |
| Manage | `.009608` | `.1399104` |626.161 /10.804% |2172.055 /72.484% |
| Balanced | `.013192` | `.4322304` |622.190 /10.982% |2178.187 /75.980% |
| Attack | `.017800` |1.0440000 |619.745 /11.131% |2186.116 /78.994% |

The error amplitude is a bounded utilization-error parameter, **not** an accident
probability. Hazard is the declared first-arrival law, not an empirically fitted
frequency. Attack gains6.416s over Manage in the short Monza scenario; Manage
gains14.061s over Attack in the long Barcelona scenario. Costs are realized
through the trajectory and tire/resource evolution, not a forced time handicap.

The limit specialist is quickest among the tested Monza archetype/mode pairs
(Attack618.074s); the smooth operator is quickest in the Barcelona scenario
(Manage2169.837s). This debug corpus supplies no global optimality/season verdict.

## One-trait comparisons

Barcelona20, Attack, risk disabled, same driver ID and actor noise, other traits
fixed at`.84`. The risk-disabled profile still retains the same control-noise
identity, avoiding an accidental reseed.

| Changed trait | Low`.78` time s | High`.96` time s | Interpretation |
| --- | ---: | ---: | --- |
| Limit exploitation |2184.892 |2192.512 | More requested force accelerates wear on this long stint; more demand is not universally a better strategy. |
| Consistency |2188.267 |2185.951 | Smaller utilization error/correction demand gains2.316s. Actual total wear can rise because useful force use increases. |
| Tire management |2188.638 |2185.392 | Correction work126.588→23.133kJ; attributed correction heat82.282→15.037kJ. Gain3.246s. |

Higher tire management slightly reduces total final wear here
(79.393→79.336%), while strongly reducing the **attributed correction** component.
It is not implemented as a blanket wear discount.

## Recoverable hesitation effect

Monza53, balanced-reference driver, fixed legal one-stop plan, seeds17/42/73.
Risk-on/off pairs keep seed, control noise and all other inputs unchanged.

| Mode | Accepted onset counts by seed | Additional elapsed time s by seed |
| --- | --- | --- |
| Manage |1 /1 /0 |2.701 /0.353 /0.000 |
| Balanced |2 /2 /1 |3.576 /2.482 /0.961 |
| Attack |5 /4 /2 |9.355 /7.090 /3.951 |

Attack remains fastest in these three full Monza debug cases. That is compatible
with the Barcelona tradeoff but does not qualify expected outcomes, rare-event
tails or every legal strategy. Three seeds are not an incident-rate study.
Some onsets occur where little torque is requested and have little effect.
Hesitation is distinct from engine-temperature protection and never relabelled
as thermal derating. No collision, spin, retirement or VSC was simulated.

## Remaining gates

Explicit V2 traits now reach the native thermal solver; historical defaults stay
reproducible. The broader #455/#456 acceptance is still open: actual demand/grip
dependence for incidents, native shared journal delivery, full process restore,
signed resource/policy admission and native/WASM/export parity.

Then #457 must fit fixed opponent contracts using real career budgets/components/
unlocks/calendars, legal full-distance fuel/finish semantics#445, optimized
concentrated engine/chassis baselines and reserved evaluations. This screen does
not prove that the current game's concentrated-allocation shortcut is resolved.
Keep the modest early cooling response accepted by the owner; do not inflate it
or fit rivals merely to create Engineer bubbles.

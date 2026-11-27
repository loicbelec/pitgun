# Interpretation and next hypothesis

## What this diagnostic establishes

The three-stop finding survives equal compound exposure. Across all eight centered family/order comparisons (two circuits × MH/HM/SM/MS), three stops beat the sampled one/two-stop schedules. Including the early/late two-stop controls does not overturn the best sampled three-stop result. The full pit cost is present: 22/44/66 s for one/two/three stops.

For MH in the forward order, three stops beat one stop by **72.449 s at Monza** and **105.786 s at São Paulo**. The corresponding driving gains are 116.449/149.786 s, offset by the additional 44 s of service. Against the centered two-stop MH plan, the extra third stop buys a smaller net 15.794/19.040 s. These are sampled comparisons, not proven optima.

| Same MH exposure | Monza: three-stop gain vs one | São Paulo: three-stop gain vs one |
|---|---:|---:|
| Frozen nominal model | +72.449 s | +105.786 s |
| Temperature-only grip loss removed | −36.564 s | −30.232 s |
| Wear-only grip loss removed | +54.794 s | +64.279 s |

Removing temperature-related grip loss reverses this ranking on both circuits. Removing wear-related grip loss reduces the advantage but does not reverse it. This supports investigating the **thermal grip/temperature trajectory interaction first**, rather than assuming the progressive wear curve alone causes the problem. It does not isolate an additive number of seconds due to each mechanism: changing grip changes speed, work, heating, fuel and subsequent wear.

## Why service is rewarding in this model

Fresh sets reset both thermal nodes to the declared 70°C preparation. In nominal MH one-stop runs, the first medium stint has a time-weighted surface mean around 124.4–124.5°C and carcass mean 138.3–140.4°C. Dividing that medium exposure into shorter stints in the three-stop recipes lowers surface means to about 110–112°C. The declared medium near-peak window is 90–120°C. Hard shows the same direction. The new sets pay a cold warm-up penalty, then spend less time with accumulated thermal state; the net thermal benefit can outweigh the extra pit cost.

The minimum temperature-grip column in the report includes the cold 70°C departure. It must **not** be read as a hot-only loss. Likewise surface and carcass temperatures are separate states; their means are not interchangeable with a real tire sensor. The reduced model has no calibrated per-wheel slip or brake/rim exchange.

The equations remain energy-accounted. In the existing two-node law, rolling/deformation work feeds heat, and air/road exchanges remove it. Grip is multiplied by a bounded Gaussian surface-temperature factor and a separate progressive wear factor. The tests preserve these heat/work laws in the counterfactuals. Conservation residuals and exact repeats validate numerical execution, **not the heat partitions, coefficients or real tire behavior**.

## What should happen next

Keep the current policy/runtime and this evidence immutable. Before fixing the nine-AI plateau, create a separate candidate identity and a small preregistered thermal sensitivity experiment. Distinguish:

1. **Heat generation/partition versus heat rejection:** inspect the rolling/deformation work and the speed/load dependence of air/road/carcass exchanges under the developed-car loads. Vary one family at a time; retain energy closure and the 70°C preparation.
2. **Temperature-to-grip response:** keep the operational window visible and the hot/cold penalty meaningful. Do not make temperature irrelevant, clamp temperatures into the window or compensate blindly by increasing pit losses.
3. **Wear response:** revisit only after the thermal behavior is understood. Preserve meaningful late-stint wear and the differences between compounds/circuits; do not require every compound or setup axis to win everywhere.

Use these same Monza/São Paulo contexts as development cases, plus entry-era controls to avoid overcooling the less powerful cars. Reserve a separate era-II circuit for validation before selecting a policy. Define measurable thermal/strategic criteria first; a physically plausible three-stop alternative need not be forbidden. Then run the broader retained era-I/II qualification with the new identity before final budget-aware imperfect AI fitting, WASM/recovery qualification and model/catalog admission.

No game coefficient, stop limit, opponent setup, saved career, Authority/Verifier contract or deployed service was changed by this diagnostic. The strategic gate in #483 remains open. This closes the diagnostic task, not the physical calibration or the release gate.

# Thermal screening interpretation

## Development choice, frozen before reserved results

The preregistered rule selects `rolling-heat-065`: change only the retained fraction of mechanical rolling dissipation entering tire heat from .75 to .65, uniformly for the three compounds. The rest is explicitly booked as direct environmental dissipation. Mechanical work, drag/resistance, deformation partition, tire windows, temperature/wear grip functions, driver and engine laws remain intact. This is an energy-accounted coefficient hypothesis, not a new heat source, a forced temperature target or measured Pirelli identification.

The source/protocol commit is `d7f56da`; selection and the complete development archive were committed in `9bb5a9c` before reserved execution. Selection depends only on development results. The surface-air trial also meets the declared criteria but has a worse hot-exposure score. The carcass-air trial has the lowest developed-car hot exposure but fails **all four entry-grip controls** and is rejected. Choosing solely the best developed-car temperatures would therefore have selected a problematic policy.

With the selected policy, post-warm-up MH-one hot exposure falls from 75.5% to 50.5% at Monza and 73.5% to 48.8% at São Paulo (relative reductions around 33–34%). Surface means fall from 128.3/127.0°C to 119.4/118.9°C, while mean temperature-grip factors rise from .880/.886 to .945/.943. Hot exposure remains substantial; the model does not erase temperature management or force every sample inside the window.

The tradeoff is explicit: the entry cars run about 4–5°C cooler. Their mean temperature-grip factors decrease by roughly .026–.029 absolute in the sampled controls, within the declared .03 screening guard. These points are model grip factors, **not a percentage of lap-time performance**. Montreal entry is near the guard; broader cold/track/driver/setup checks remain important before any adoption. The screen does not establish that this tradeoff is desirable across every era, setup or compound.

## Strategy consequence

The criterion did not require any particular stop count to win. Nevertheless, among the centered equal-exposure MH samples, the selected policy changes the preferred developed-car schedule from three stops to two:

| Circuit | Reference: two minus three stops | Selected: two minus three stops |
|---|---:|---:|
| Monza | +15.794 s (two slower) | −22.587 s (two faster) |
| São Paulo | +19.040 s (two slower) | −27.843 s (two faster) |

Every stop still costs 22 s. These are sampled plans, not optimized stop timings or proof that three stops can never win. Other compound families, budgets, setups, seeds and eras remain outside this small screen. Do not cap only the player or re-use an old opponent-calibration result with this policy.

## Next gate

Reserved validation passes without changing the candidate: Mexico MH-one hot exposure falls from 55.9% to 15.2% (about 73% relative reduction), and the Zandvoort entry mean temperature-grip loss is .024 absolute, within the .03 guard. All six fresh input/full-result repeats match exactly. Across both phases: **54 full GP / 3,360 laps, zero numerical failures**, exact boundary carry and maximum tire balance residual about 6.69e-9 J.

Mexico also illustrates the limits of extrapolating the development circuits: the reference matched MH two-stop recipe already beats its three-stop recipe. With the selected policy the sampled one/two-stop finishes differ by only **.173 s**, while three stops are slower. The earlier seven-circuit counterexample used other compound sequences/timings; it never established a universal three-stop optimum. This experiment supports circuit-dependent alternatives, not the same strategy everywhere.

Read `validation-v1.json` and `RESULTS_V1.md` for the separately frozen Mexico/Zandvoort validation and repeat outcome. A failed reserved check rejects qualification; it does not authorize switching to another trial after looking at the result. A successful reserved screen yields a candidate for broader era-I/II qualification, including all three compounds, setup-axis significance, meaningful engine cooling, legal gearing, full race service and diverse strategies. Only after that comes budget-aware imperfect nine-AI calibration, native/WASM/recovery parity and separately versioned catalog/model/runtime admission.

No existing game policy, runtime, local saved career or deployed service is changed here. The physics qualification/release gate stays open even if this bounded screen passes.

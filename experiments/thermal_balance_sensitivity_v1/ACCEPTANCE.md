# Thermal balance sensitivity V1 — preregistered selection and validation

Follow-up to #485/#483/#479, before final nine-AI calibration. Offline experiment only. Preserve the nominal policy, runtime, prior probes and archives, saved careers and deployed services. No solver equation, tire window, wear law, compound grip multiplier, service cost, engine, fuel law, driver, boundary method or observation spacing changes. Initial fuel 100 kg; initial/replacement surface and carcass 70°C.

## Hypotheses, not identified tire coefficients

The retained developed-car runs dissipate about twice the tire heat power of the entry car. Existing source partitions and exchanges are reduced-model hypotheses. Test only these three one-factor changes, uniformly across the three compounds:

- `rolling-heat-065`: rolling work retained as tire heat .75 → .65. Removed retention is explicitly accounted as direct environmental dissipation. Mechanical resistance, wear generation law and deformation partition remain unchanged.
- `surface-air-120`: surface air-speed conductance .9 → 1.2 W/K/(m/s). Other exchanges and heat sources remain unchanged.
- `carcass-air-45`: carcass-air conductance 30 → 45 W/K. Other exchanges and heat sources remain unchanged.

Every candidate has a separate immutable policy file/hash. The published C3/C4/C5 names and prior literature motivate a thermal window and layered model; **these trial coefficients and acceptance bounds are Pitgun hypotheses, not Pirelli measurements**. First inspect this bounded family; do not silently introduce a combined candidate or extend the grid if none qualifies.

## Development — 40 full GP

Reference plus three candidates. Each runs actual developed era-II Craft Monza/São Paulo MH with 1/2/3 stops, balanced sliders/driver, 16 discretionary points plus actual 8 bonuses. Preserve exact per-compound lap totals across stop counts. Controls: era-I entry Monza/Silverstone SM one stop with `[2,2,2,2]`; era-II entry Monza/Montreal SM one stop with `[3,3,3,2]` plus actual bonus floor. Same seed 73013 and installed components throughout. Each policy solves ten races, one serial process at a time; compilation uses two jobs.

Measure true driving-time-weighted hot/cold/window exposure, temperature grip, surface/carcass states, wear, fuel, thermal ledgers and service times. For selection metrics exclude the first **three laps of each stint** for every policy equally; retain all warm-up laps in evidence and all race totals. No temperature clipping or artificial warming.

## Selection rule — before reserved contexts

Require complete numerical success, fixture/component/fuel binding, exact service accounting, continuous boundaries and thermal conservation under prior tolerances. Require exact reference input/result bridges to the two prior developed-car MH-half runs. A candidate is eligible only if:

1. Hot exposure (surface above its declared compound window, weighted by post-warm-up driving time) in **each** developed-car MH one-stop development context decreases by at least 20% relative to reference.
2. In **each** entry control, post-warm-up mean temperature grip falls by no more than .03 absolute relative to reference. This guards against solving developed-car overheating by overcooling the entry cars.
3. All ten development runs remain below provisional ceilings of 155°C surface and 165°C carcass. These are screening limits for this reduced-model experiment, not a universal tire safety specification.

Among eligible candidates choose the lowest sum of the two reference-normalized hot-exposure fractions; break exact ties by policy ID. Save selection, all eligible/rejected candidates and reasons, development-archive hash and selected-policy hash **before** any reserved run. No stop-count victory is an eligibility criterion. No eligible candidate means stop and report rejection; no post-hoc thresholds or extra candidates.

## Reserved validation — at most 14 further full GP

Freeze selection first. Run reference and that candidate only: era-II developed Mexico MH 1/2/3 stops (six GP), plus era-I entry Zandvoort SM one stop (two GP). Mexico is reserved for this candidate-selection experiment, not an independent unseen-circuit validation of all previous Pitgun research. Apply the same numerical/ceiling checks, 20% hot-exposure improvement to Mexico one stop, and .03 entry-grip guard to Zandvoort. Do not choose another candidate or tune coefficients on a failed validation.

Six additional fresh repeats must match physical-input and full-result digests: reference/selected developed MH-one on Monza, São Paulo and Mexico. Maximum campaign is **54 full GP**. A validation failure remains visible and prevents selection from being qualified. Preserve all failures and accepted prefixes, binary/source/fixture/policy/plan pins, immutable phase-specific caches and frozen selection hash. No concurrent native solves.

## Release decision

Report every candidate, exposure changes and all 1/2/3-stop comparisons. Temperature and wear must still affect grip. Three stops may be competitive; do not force an optimum, increase pit costs to hide it or restrict only the player. A successful screening yields a candidate for broader era-I/II qualification, not an admitted game model, final AI fit, WASM proof or deployment. Broader qualification remains required before catalog/runtime adoption.

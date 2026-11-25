# Three-compound operating-window candidate

Owner-approved lot 1, 25 November 2026. Tracking: [#479](https://github.com/loicbelec/pitgun/issues/479), tyre umbrella #421, corrected local integration #475/#477.

This is an opt-in native candidate on top of the corrected gearbox. The existing model 0.16.1/catalog 1.10.1, owner preview 5251, hosted services and calibration caches are unchanged. No old result is relabelled as evidence for these coefficients. The nine inherited AI preparations are not qualified by these player-only runs.

## Model and compilation

Exactly soft/C5, medium/C4 and hard/C3. Public compound names identify the inspiration, not a measured Pirelli parameter set or historical-era specification. `candidate-v1.json` is explicit input to `compile_tire_operating_v1`; no existing runtime selects it. The compiled wrapper binds the complete policy digest as well as the existing performance/thermal compilation. Future runtime admission must bind this identity and rebuild the artifact; it cannot silently reuse 0.16.1.

The existing Gaussian is used: `g(T) = max(g_min, exp(-((T-Topt)/sigma)^2))`. A declared near-peak interval [L,H] means g >= .95, with Topt=(L+H)/2 and sigma=(H-L)/(2 sqrt(-ln(.95))). The existing recorded tire-window detector derives the same boundaries. The performance copy of sigma is checked for agreement, rather than allowed to disagree with the window. Heat/wear duplicates are also rejected if inconsistent. Every set is validated before execution, even an unused future pit replacement.

Initial design hypotheses: soft85..115°C, medium90..120°C, hard95..125°C. These numbers are **Pitgun hypotheses**, not published modern Pirelli operating windows. Mount/replacement remain70°C. They need bounded native and later browser/strategy qualification, not a demand that every segment lie inside a fixed target.

Two existing thermal capacities remain:

- Cs*dTs/dt = Qsurface + G*(Tb-Ts) - Hair(v)*(Ts-Tair) - Hroad*(Ts-Troad).
- Cb*dTb/dt = Qbulk - G*(Tb-Ts) - Hbulk*(Tb-Tair).
- Qsurface = fs*fd*Pdeformation; Qbulk = fr*Proll + (1-fs)*fd*Pdeformation.

The diagnostic separates window-only, retained heat, speed-dependent air exchange and surface/carcass coupling variants. The retained candidate uses rolling/deformation heat fractions .75/.9, deformation surface share .8, inter-node conductance400W/K and air-speed conductance .9W/K/(m/s). These are hypotheses. The high-retention .85/.7 air-speed trial is retained as an overheating counterexample. Rolling/deformation mechanical work is already debited in the energy ledger. Increasing the fraction retained by tires reduces the explicitly counted direct-to-environment remainder; it does not create additional mechanical work. No added braking energy or ideal lateral-force*forward-speed heat, no imposed temperature floor/target and no observation-rate change. Genuine slip, brake/rim heat transfer and per-wheel measurements remain outside this two-node model.

Heat partition coefficients are reduced-model hypotheses, not identified values. Keep source-partition and boundary-exchange experiments distinguishable. Preserve work-based progressive wear. Adjust gain with sigma squared to preserve each compound's previous wear penalty per degree squared; changing the optimum still changes wear at a given temperature, and must be qualified.

## Bounded execution

`python3 experiments/three_compound_thermal_v1/run.py --target-dir /ABSOLUTE/LOCAL/CARGO/TARGET --output experiments/three_compound_thermal_v1/results/candidate-native-v1.json`

Use `--policy experiments/three_compound_thermal_v1/high-retention-trial-v1.json` with a different output to reproduce the overheating counterexample. The runner builds offline with two compiler jobs and solves one car at a time. It records source and binary hashes and retains failures. No workers are launched remotely.

Two retained contexts (Monza I and Montreal II), three compounds, five laps, five controlled variants; only the player is solved. Extended mode repeats each candidate exactly, adds cold35/hot140/replacement/full-GP cases in both contexts and three reserved Silverstone-II compounds. Total41 reported cases plus6 exact repeats (47 solves /348 driven laps per policy). The final operating candidate is fixed before reserved-context execution; no subsequent fitting on that context. See ACCEPTANCE.md. This is not a nine-car competition, optimization, full-GP strategy ranking or validation of unseen tracks. Reports retain per-lap surface/carcass/grip/rise/fall measures, wear and energy ledger; provenance records binary and source hashes. Failures remain failures.

## References and adoption limits

- [Pirelli 2025 season report](https://press.pirelli.com/the-2025-formula-1-season-in-numbers/): C3/C4/C5 lead usage (93,493 / 91,595 / 66,255 km). Basis for the trio only, not thermal coefficients.
- [Mercedes, Temperature Talk](https://www.mercedesamgf1.com/news/insight-temperature-talk): narrow operating region and loss of grip below/above, increased wear when hot. Principle, not transferable coefficients.
- [Schabauer, Scherndl & Lex (2024)](https://link.springer.com/chapter/10.1007/978-3-031-66968-2_29), publisher HTML equations1–7 and sections1–2: distinct thermal layers, source partition, hysteresis/sliding, signed air/road exchanges. We retain our own two-node reduction and dissipation closure; no claim to implement their measured slip/force characteristics or calibrated parameters.
- [FIA/Pirelli Bahrain2025 prescriptions](https://www.fia.com/system/files/decision-document/2025_bahrain_grand_prix_-_event_notes_-_pirelli_preview.pdf): distinguish measured external surface from blanket controller; retain the explicitly hypothesised homogeneous70°C preparation of the previous policy. Not a claim that carcass/preheat/operating temperatures are interchangeable.

## Follow-up gates

Native cold/hot/stop/continuity, setup/driver/track sensitivity and whole-GP zero/one/two-stop comparisons (with three-stop counter-tests) precede AI fitting. Then new model/catalog/artifact, native/WASM/recovery/replay and separately authorized local activation. No need to freeze AI optima: selected setups should be competitive and imperfect.

## Strategic follow-up

The owner accepts circuit-dependent operating behavior as part of the challenge; there is no requirement to equalize temperatures or make every compound competitive everywhere. [Era-I qualification](../era1_strategy_qualification_v1/RESULTS_V1.md) now retains144full GP/8496laps under this same frozen candidate. It supports meaningful sampled setup/stop/gearing tradeoffs; broader physical identification, eraII, budget-aware AI fitting and runtime admission remain separate gates. The earlier dated thermal trial and its criteria are preserved as historical evidence.

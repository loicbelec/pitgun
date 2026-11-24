# Owner feedback: RPM, tire temperatures, hysteresis and equal-budget competitiveness

This read-only audit follows the corrected 0.16.1 preview. It does not change active physics, tires, AI, budgets, runtime, catalog or owner saves. No new fit/sweep.

## Current implementation

- Engine redlines are authored per resource: V8 1960 11,000 RPM, V8 1970 13,000 RPM. Energy advance caps speed for the selected ratio at `n_max * TAU * wheel_radius / (60 * ratio)`. Upshift/downshift mechanical defaults remain 11,000/5,500 RPM with a 0.050 s shift interval. The corrected energy equality guard retains these separate thresholds and adjacent selection; it does not remove hysteresis. Current common shift thresholds should eventually be reviewed against each engine map, without assuming the redline is necessarily the best shift point.
- Displayed tire temperature is the aggregate equivalent surface node, distinct from the modeled carcass and from local IR hot spots. Mount/replacement surface and carcass are initialized to 70 C. Heating currently comes from accounted rolling/deformation dissipation and losses to air/road; this is not an identified four-wheel slip/rim/brake-transfer model.
- Current compound optima are soft90/medium95/hard100 C. The inherited tire-performance compiler expands Gaussian sigma to117.8/130.2/148.8 C, respectively. These sigma parameters are NOT operating-window spans. With `exp(-((T-Topt)/sigma)^2)`, the 95%-peak windows are [63.32,116.68], [65.51,124.49], [66.30,133.70] C. This is much more permissive of cold tires than a narrow F1-type working-window objective; numeric coefficient fitting is not physical identification.
- Every AI has the same discretionary budget within its era: I8/II16. Era-II component offsets add [2,1,4,1] in aero/chassis/engine/cooling, giving 24 effective points plus the upgraded V8/aero baseline. The authored Craft Engineering snapshots have I-entry8+0, I-exit11+1, II-entry11+1 with old engine, II-exit16+8 with upgraded engine. Therefore rivals are NOT always equal to the current player's available budget/technology.
- Player driver reference traits are 0.84/0.84/0.84 (limit exploitation/consistency/tire management). Franz0.84/0.88/0.84 and Charles0.87/0.83/0.83 are intended favorites, not a hard-coded finishing order. Corrected-model hierarchy and player winning latitude still require qualification. Never use the old gear-locked campaign's win rates as that proof.

## Bounded diagnostic

[Retained report](results/owner-feedback-monza8-v1.json), reproducible with `cargo run --release -p pitgun-racing-simulator --example career_feedback_probe`: one Monza-I context, seed1960, 53 laps, 8 effective points per actor, same baseline car. Nine fixed AI races computed once plus six prespecified player setups; all player cases use the same soft26/medium27 one-stop plan. This does not optimize strategy or search the whole setup space. No traffic interaction is modeled; actors are independent under these fixed plans.

| Player case | Allocation A/C/E/Cool | Gear slider | Position | Surface mean laps3–5 | Carcass mean laps3–5 |
|---|---|---:|---:|---:|---:|
| balanced | 2/2/2/2 | 0.5 | 8 | 67.85 C | 76.52 C |
| engine/chassis | 0/4/4/0 | 0.5 | 10 | 67.42 C | 76.11 C |
| engine/chassis/cooling | 0/3/3/2 | 0.5 | 8 | 67.85 C | 76.53 C |
| chassis/cooling, long | 0/6/0/2 | 1.0 | 8 | 67.61 C | 76.30 C |
| chassis/cooling, mid | 0/6/0/2 | 0.5 | 8 | 67.80 C | 76.49 C |
| mixed, long | 2/3/2/1 | 1.0 | 10 | 67.45 C | 76.15 C |

Best tested player is 50.701 s behind the leader over the GP (about0.96 s/lap). Luis leads this specific fixed field, Charles second, Franz third. This establishes neither an impossible-to-win field nor a sufficient winning opportunity: six unoptimized preparations/one plan/one seed are insufficient. It does confirm a cold, weakly varying surface response on this case and a need to establish winning-capable admissible player preparations after physical corrections.

## References and next gates

- [Pirelli 2022 presentation](https://press.pirelli.com/2022-pirelli-motorsport-season-presentation/) documents the move to70 C preheating; this is not the operating optimum or a floor during running.
- [Pirelli 2019 compound ranges](https://press.pirelli.com/whats-new-with-pirellis-2019-formula-1-tyres/) publishes C5 85–115 C through C1 110–140 C for that generation. Historical, not universal/current compound targets.
- [Mercedes temperature explanation](https://www.mercedesamgf1.com/news/insight-temperature-talk) describes the narrow grip window and sensitivity to setup/driving.
- [Mercedes official surface/bulk explanation](https://www.linkedin.com/posts/mercedes-amg-petronas-formula-one-team_what-impact-do-temperatures-have-on-an-f1-activity-7115015810992988160-kJk7) distinguishes rapid surface variation from slower bulk warm-up. A localized130–150 C spot is not a target for a four-tire aggregate node.
- [FIA/Pirelli Bahrain2025 notes](https://www.fia.com/system/files/decision-document/2025_bahrain_grand_prix_-_event_notes_-_pirelli_preview.pdf) distinguish actual tread/sidewall measurements from controller set points.

Recommended order: inspect tire heat generation/loss ledger and thermal response to real load before narrowing grip curves; define documented compound-specific targets for the game's chosen tire abstraction; retain cold exits/straights/safety-car cooling rather than clamping to80/100 C; test underheated/useful/overheated regimes and wear, then strategy and AI. Keep these targets explicitly game-model hypotheses, not measured industrial identification.

Define what equal budget means before refitting: available preparation envelope (not spent points), component bonuses and car architecture are separate. Do not silently cancel HQ progress by mirroring every purchase or weaken rivals when the player leaves points unspent. Fixed event/benchmark equality is straightforward; career progression conventions need to stay explicit. Require feasible competitive/winning player preparations and losing poor preparations under equal conditions, without forcing any favorite to win. Further campaigns remain local and one era at a time after these gates.

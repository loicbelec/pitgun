# Racing Opponent Policy V2

Status: human-reviewed Competitive policy proposed for issue #167.

## Decision

Pitgun now has enough governed evidence to publish a bounded policy without
claiming perfect calibration. V2 converts the existing deterministic game
mechanics into catalog-owned balance data and records which choices are backed
by experiments, authored defaults, or fallback behavior.

The policy supports game progression eras 1 through 5. Eras 6 and 7 fail
closed: restoring them requires the separate late-era engine and powertrain
redesign tracked in `pitgun-game#160`.

## Runtime behavior

One public race context and seed deterministically produce nine opponents:

- two front-runners;
- four midfield opponents;
- three challengers.

Every opponent stays within 95–105% of the event's common development budget.
The five existing development styles remain as explainable allocation profiles,
not hidden pace multipliers. Circuit baselines, style biases, setup mutation,
role mix, budget tolerance, and strategy preferences now live in the policy
rather than requiring TypeScript balance constants.

The generator must not inspect the player's selected setup or stint plan.
Unsupported circuits use the explicit `DEFAULT` baseline. Unsupported eras are
rejected rather than silently mapped to a known era.

## Evidence

| Question | Governed evidence |
| --- | --- |
| Current opponent behavior | `racing-opponent-audit-2026-v1`, 180 successful runs, MLflow `2f5b0a37a53d4ac7b2bbf59f180bf186` |
| Strategy effect | 90 successful runs and 45 exact pairs, MLflow `afa53d82213a43708d068956c59c6f28` |
| Economy-backed budget response | 135 successful runs and 45 exact triplets, MLflow `ea3e8ae16fba45e4878656219b421359` |
| Early marginal allocation | 135 successful runs and 60 paired comparisons, MLflow `5668a48da19a4263828d3ef712addd0f` |
| Progression anchors | public game-economy artifact `sha256:1e5a082ff05b8c66d43ad0d69306af608c2b9fd4253a4d479d3c0f9c03daf23c` |

The strategy evidence prefers late one-stop in early and middle progression.
At late progression it remains preferred at Monza and Suzuka; the deterministic
fallback is balanced one-stop elsewhere. A 25% seeded alternate selection
preserves legible grid diversity without reading the player's strategy.

## Honest limitations

Only Budapest, Monaco, Monza, Singapore, and Suzuka have governed
circuit-specific evidence in this policy generation. Other published baselines
are authored public defaults and are marked as such. The development styles
are bounded diversity profiles, not claims of global optimality. The marginal
campaign selected no allocation profile: it found a locally dominant chassis
response, a smaller engine response, no cooling response without thermal
derating, and a slightly negative aero response. Those findings stay tracked
under #229 and are deliberately not converted into policy weights.

These limitations do not block a reviewed V2: deterministic game fixtures and
full staging weekends are the next acceptance gate. Any later calibration
creates a new immutable policy version.

## Publication

The candidate is stored in catalog release `v1.3.0`. Adding the immutable
release does not promote the public mutable pointer. Promotion remains a
separate reviewed operation after the game consumer, golden grids, Hosted
Verification, and staging weekend tests pass.

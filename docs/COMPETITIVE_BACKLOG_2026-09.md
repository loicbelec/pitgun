# Competitive backlog — September 20, 2026

Planning baseline, not implementation or release evidence. Project fields are authoritative;
this table records the initial Ready/Backlog assignment. Ready means a scoped first slice,
not that downstream integration is unblocked. Existing accepted experiments retain their
original model/catalog scope. No new services, coefficients or DNS are activated.

Design: [framework plan](COMPETITIVE_RACING_AND_ENGINEER_PLAN.md).

| Component | Work | Priority | Initial status | Dependencies / coordination |
| --- | --- | --- | --- | --- |
| framework | [fix(racing): define bounded throttle and consistent delivered-power telemetry](https://github.com/loicbelec/pitgun/issues/419) | High | Ready | See issue acceptance |
| framework | [analysis(racing): validate cooling as a measurable full-race tradeoff](https://github.com/loicbelec/pitgun/issues/420) | High | Ready | See issue acceptance |
| framework | [analysis(racing): validate tyre thermal wear and competing stint strategies](https://github.com/loicbelec/pitgun/issues/421) | High | Ready | See issue acceptance |
| framework | [feat(competition): specify immutable rules manifests and eligibility evidence](https://github.com/loicbelec/pitgun/issues/422) | High | Ready | See issue acceptance |
| framework | [feat(benchmarks): preregister reproducible Engineer evaluation and evidence bundles](https://github.com/loicbelec/pitgun/issues/423) | Medium | Ready | [pitgun #422](https://github.com/loicbelec/pitgun/issues/422), [pitgun #419](https://github.com/loicbelec/pitgun/issues/419), [pitgun #420](https://github.com/loicbelec/pitgun/issues/420), [pitgun #421](https://github.com/loicbelec/pitgun/issues/421) |
| game | [roadmap(competition): coordinate balance, verified challenges, Engineers and community feedback](https://github.com/loicbelec/pitgun-game/issues/266) | High | Backlog | [pitgun #419](https://github.com/loicbelec/pitgun/issues/419), [pitgun #420](https://github.com/loicbelec/pitgun/issues/420), [pitgun #421](https://github.com/loicbelec/pitgun/issues/421), [pitgun #422](https://github.com/loicbelec/pitgun/issues/422), [pitgun #423](https://github.com/loicbelec/pitgun/issues/423) |
| game | [feat(leaderboard): compare real race championships under common rules](https://github.com/loicbelec/pitgun-game/issues/267) | Medium | Backlog | [pitgun #422](https://github.com/loicbelec/pitgun/issues/422) |
| game | [feat(leaderboard): explain simulation lineage through progressive Data and proof](https://github.com/loicbelec/pitgun-game/issues/268) | Medium | Ready | [pitgun #422](https://github.com/loicbelec/pitgun/issues/422) |
| game | [feat(feedback): add minimal private player feedback intake and triage](https://github.com/loicbelec/pitgun-game/issues/269) | Medium | Ready | See issue acceptance |
| site | [feat(community): publish a moderated player-feedback board](https://github.com/loicbelec/pitgun-site/issues/1) | Medium | Backlog | [pitgun-game #269](https://github.com/loicbelec/pitgun-game/issues/269) |
| site | [feat(lab): present reproducible competition and Engineer benchmark evidence](https://github.com/loicbelec/pitgun-site/issues/2) | Medium | Backlog | [pitgun #423](https://github.com/loicbelec/pitgun/issues/423), [pitgun-game #268](https://github.com/loicbelec/pitgun-game/issues/268) |
| api | [docs(api): inventory pitgun.io routes and preserve telemetry registry ownership](https://github.com/loicbelec/pitgun-api/issues/1) | Medium | Ready | See issue acceptance |
| infra | [feat(pitgun): design scoped Engineer ingress, quotas and service isolation](https://github.com/loicbelec/infra-vps/issues/81) | Medium | Backlog | [pitgun-api #1](https://github.com/loicbelec/pitgun-api/issues/1) |
| infra | [feat(pitgun): qualify isolated model workers and durable benchmark artifacts](https://github.com/loicbelec/infra-vps/issues/82) | Medium | Backlog | [pitgun #423](https://github.com/loicbelec/pitgun/issues/423), [infra-vps #81](https://github.com/loicbelec/infra-vps/issues/81) |
| framework | [roadmap(racing): coordinate competitive physics and opponent calibration](https://github.com/loicbelec/pitgun/issues/413) | High | Backlog | [pitgun #419](https://github.com/loicbelec/pitgun/issues/419), [pitgun #420](https://github.com/loicbelec/pitgun/issues/420), [pitgun #421](https://github.com/loicbelec/pitgun/issues/421) |
| framework | [analysis(racing): reconcile gameplay progression with physical development saturation](https://github.com/loicbelec/pitgun/issues/229) | High | Ready | [pitgun #419](https://github.com/loicbelec/pitgun/issues/419), [pitgun #420](https://github.com/loicbelec/pitgun/issues/420), [pitgun #421](https://github.com/loicbelec/pitgun/issues/421), [pitgun #422](https://github.com/loicbelec/pitgun/issues/422) |
| game | [analysis(gameplay): validate early-era opponent pace and setup benefits](https://github.com/loicbelec/pitgun-game/issues/234) | High | Ready | [pitgun #419](https://github.com/loicbelec/pitgun/issues/419), [pitgun #420](https://github.com/loicbelec/pitgun/issues/420), [pitgun #421](https://github.com/loicbelec/pitgun/issues/421) |
| game | [feat(gameplay): make Pit Wall competitors varied and less predictable](https://github.com/loicbelec/pitgun-game/issues/139) | High | Backlog | [pitgun #419](https://github.com/loicbelec/pitgun/issues/419), [pitgun #420](https://github.com/loicbelec/pitgun/issues/420), [pitgun #421](https://github.com/loicbelec/pitgun/issues/421) |
| game | [leaderboard: isolate economy and simulation cohorts before seasonal publication](https://github.com/loicbelec/pitgun-game/issues/259) | High | Ready | [pitgun #422](https://github.com/loicbelec/pitgun/issues/422) |
| game | [feat(challenges): share a deterministic head-to-head Pit Wall confrontation](https://github.com/loicbelec/pitgun-game/issues/183) | Medium | Backlog | [pitgun #422](https://github.com/loicbelec/pitgun/issues/422) |
| game | [roadmap(engineer): deterministic simulation control and Bring Your Own Engineer](https://github.com/loicbelec/pitgun-game/issues/161) | High | Backlog | [pitgun #423](https://github.com/loicbelec/pitgun/issues/423), [pitgun #422](https://github.com/loicbelec/pitgun/issues/422), [infra-vps #81](https://github.com/loicbelec/infra-vps/issues/81), [infra-vps #82](https://github.com/loicbelec/infra-vps/issues/82) |
| game | [feat(engineer): delegate in-race pit and tyre decisions to Gemini](https://github.com/loicbelec/pitgun-game/issues/253) | Medium | Backlog | [pitgun #423](https://github.com/loicbelec/pitgun/issues/423) |
| game | [feat(lab): benchmark recorded race-engineer strategies locally](https://github.com/loicbelec/pitgun-game/issues/254) | Medium | Backlog | [pitgun #423](https://github.com/loicbelec/pitgun/issues/423), [pitgun #422](https://github.com/loicbelec/pitgun/issues/422), [infra-vps #82](https://github.com/loicbelec/infra-vps/issues/82) |
| game | [feat(engineer): prepare provider-independent gateway on pitgun.io](https://github.com/loicbelec/pitgun-game/issues/264) | Medium | Ready | [pitgun-api #1](https://github.com/loicbelec/pitgun-api/issues/1), [infra-vps #81](https://github.com/loicbelec/infra-vps/issues/81), [infra-vps #82](https://github.com/loicbelec/infra-vps/issues/82) |
| framework | [feat(control): add deterministic Racing pit and tyre decision boundaries](https://github.com/loicbelec/pitgun/issues/415) | Medium | Backlog | [pitgun #422](https://github.com/loicbelec/pitgun/issues/422), [pitgun #423](https://github.com/loicbelec/pitgun/issues/423), [pitgun #421](https://github.com/loicbelec/pitgun/issues/421) |
| framework | [feat(energy): validate external controller boundaries with a synthetic battery](https://github.com/loicbelec/pitgun/issues/416) | Medium | Backlog | [pitgun #423](https://github.com/loicbelec/pitgun/issues/423) |
| framework | [feat(engineer): expose scoped observation polling for external controllers](https://github.com/loicbelec/pitgun/issues/417) | Medium | Backlog | [pitgun #422](https://github.com/loicbelec/pitgun/issues/422), [pitgun #423](https://github.com/loicbelec/pitgun/issues/423), [infra-vps #81](https://github.com/loicbelec/infra-vps/issues/81) |

## Delivery gates

- Physics/cap mapping diagnostics can start now. Final numerical allocation and opponent
  calibration require accepted response surfaces and held-out evidence; no arbitrary cap
  has been selected in this plan.
- Cohort/manifest work can run alongside diagnostics. Rank only eligible fixed-means
  executions; career resource metadata is not proof of acquisition.
- Friend weekends use three official practice slots, then one race, durable state and
  polling. A public simulator cannot prevent unofficial practice.
- A real common-calendar Championship follows the challenge/result foundation; keep
  historical hot-lap standings accurate and accessible.
- Controller contract preparation can proceed before calibration finishes. Execute the
  benchmark only after the boundary works and physical scenario preflight passes.
  Battery proof precedes freezing the public generic BYO envelope.
- Feedback intake is independent. Publish only moderator-approved records; keep raw
  player reports and internal technical triage private. Votes and automation come later.

Hosted release acceptance remains game #256 / candidate PR #257 and infra PR #80.
Monaco fuel framework #414 remains deferred and disclosed; hybrid #246 and circuit
promotion game #229 retain their separate scope. Historical campaigns #385/#388 are
not reopened or relabelled as validation of future model changes.

Ownership: framework owns physics, contracts, verification and reproducible experiment
manifests/data. Game owns UI and application services. API owns the telemetry registry.
Site owns public Lab/community presentation. Infrastructure owns ingress, isolated
operator-controlled workers and durable artifact storage. No additional repository is
needed now; tooling, blueprints and Petit Courrier remain outside this task.

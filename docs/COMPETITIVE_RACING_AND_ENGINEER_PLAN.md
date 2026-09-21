# Competitive Racing and external-controller evaluation

September 21 sequencing update: the [detailed progressive simulation plan](https://github.com/loicbelec/pitgun-game/blob/docs/progressive-simulation-plan/docs/design/PROGRESSIVE_SIMULATION_DELIVERY_PLAN.md)
is awaiting owner approval. First deliver game #273/#274 (buffered fixed-strategy
start and safe recovery), then #415's real reference-controlled boundary, then game
#253/#275 (Gemini actions and contextual advice). Physics diagnostics below may run
independently and still gate benchmark claims. Hosted deployment is not part of this
planning pass; public BYO and the battery proof retain their later gates.


20 September 2026. Planning scope: resume model/decision-surface audits after acceptance of the local game UI. No coefficient, published resource, execution schema or deployment changes in this delivery. This document refines #413 and the external-controller plan; accepted historical campaigns remain valid only for their pinned artifacts.

## Ownership and sequence

Racing Solver owns physical equations; Racing Simulator owns state/decision boundaries; Racing contracts/policy own admissibility. Generic runtime/contracts own only demonstrated domain-neutral lifecycle and evidence primitives. The game backend owns participants, challenges, attempt grants and score projections. Infrastructure owns ingress, credentials, isolation and job operations. Databricks/Python orchestrate the Rust workload, never reimplement its physics.

1. Audit throttle/power, thermal/cooling, tyres and effective development ranges on exact published inputs and separate candidates.
2. Define immutable competition eligibility and scenario manifests. The game projects verified results within a cohort; product version alone is not an identity.
3. Validate fixed-condition friend weekends and championship scoring against that contract.
4. Prove deterministic in-race pit/tyre control with #415; Gemini integration is a client. Preflight the physical scenario before benchmarking decision quality.
5. Freeze an evaluation campaign, compare reference/heuristic/frontier/lightweight controllers, then expose BYO polling after #416's independent battery-domain proof.

The #415 protocol prototype may proceed alongside calibration. Model-quality benchmark claims and public rankings wait for validated scenarios. Monaco #414 remains an explicit fuel/preflight failure until separately resumed; do not hide it or introduce a frontend fuel override. Hybrid #246 remains separate when not required to correct current non-hybrid telemetry.

## Physics and decision-surface acceptance

- Throttle is currently reconstructed with a bound of 1.2; power uses the same ratio. Establish requested/available/delivered power on consistent shaft/wheel bases and units before changing the ratio. A pedal channel belongs to [0,1]; a utilization ratio with other semantics must have another channel. Golden cases include zero load, traction limitation, gear changes, derating, braking and supported recovery modes. Preserve historical channel provenance.
- Thermal mechanics and cooling-drag profiles already exist. The reviewed Silverstone family campaign demonstrated interior modern cooling optima but used an experimental 130 kg reservoir; unchanged historical V8s did not derate. This is bounded evidence, not universal game acceptance. Check actual power-unit/component/profile bindings, fuel, full distance, era/budget/bonus anchors and held-out circuits. Avoid both universally free zero cooling and universally optimal maximum cooling without forcing an artificial historical mechanic.
- Tyre state already includes temperature and workload/compound wear. Revisit initialization, compound changes, thermal grip and persistent degradation together. Earlier V7 tyre screening retained a REFINE verdict because late stops and hard tyres dominated. Report crossover and stop-window behavior rather than tuning to a predetermined number of stops.
- Reconcile gameplay free points, acquired offsets, per-axis caps and physical response (#229). Compare bounded-specialization, sparse free-budget and diminishing-return candidates; do not pick arbitrary slider caps or apply tighter constraints only in the browser. Equal-budget competitors must satisfy the same published admissibility profile.

Use calibration/held-out scenario partitions, paired seeds, full-distance preflight, zero/uniform/naive/current-policy/informed references, marginal benefit/saturation/non-dominance and failures separately from valid pace. Register acceptance thresholds after a bounded pilot and before the reserved campaign. Reuse experiments/manifests, exact source/profile digests, Delta versions and MLflow evidence. Archive ACCEPT/REFINE/REJECT human review. Promote model/catalog/Authority/Verifier/browser together only after acceptance; old replays stay pinned.

## Competition manifest (proposed new version, not an implemented schema)

Separate `competition_id` (event), `cohort_id` (comparable rules), server participant/attempt identity and deterministic run identity. A content-addressed manifest binds:

- domain, schema version, model/solver/runtime/numerical profile and artifact digests;
- catalog/simulation-pack bytes and resolved car/component/tyre/track/profile identities;
- distance, declared conditions/exogenous series and seed or precommitted hidden-seed policy;
- driver/opponent policy and fixed common field;
- total preparation budget, per-axis/effective-offset bounds, eligible technology and continuous setup ranges;
- official practice slots, race-attempt/retry policy, configuration lock boundary, observation/action permissions;
- scoring, penalties, ties, DNF/DNS, deadline and reveal policy.

Authority validates the applicable manifest and participant grant. Verifier binds final applied events and execution output, including the manifest identity, before application projection. A local career UUID, nickname or submitted upgrade list does not attest economic eligibility. Begin with server-defined fixed means; authoritative progression is a separate product requirement. Competition rules, opponent policy and the simulator's physical model are distinct artifacts.

Preserve existing contracts. Specify version negotiation and initial authorization/final evidence migration before implementation. Test cross-cohort overwrite isolation, forbidden offsets/components, duplicate/conflicting attempts and score derivation. No browser-provided winner, time or verification flag is authoritative.

## Controller/evaluation protocol

#415 remains the first actual intervention: frozen observation at a real simulation boundary, bounded keep/pit/compound proposal, domain validation, receipt, applied event and resume. The current dynamic completed-input API and the browser playback cursor do not supply this behavior. No future solved lap, hidden strategy or RNG state enters an observation. Game #177 separately owns driver/aggression commands.

The host owns wall-clock deadlines and quotas, while the domain owns simulated application boundaries and legal fallback. If timeout selects fallback, record that selection. Replay consumes recorded actions without inference; rerunning a model is a new experiment. Identical seed/temperature does not guarantee identical LLM responses.

Register two tracks:

| Track | Frozen | Allowed differences |
| --- | --- | --- |
| Model comparison | Prompt/task framing, observations, tools, memory policy, decision windows, output schema, fallback, attempts | Declared provider adapter, exact model revision/runtime/quantization |
| Engineer competition | Scenario, capabilities, information/tool budgets, score and official attempts | Declared prompts, memory, agent policy and composed controller |

Before reserved evaluation, freeze scenario/seed split, repetitions per stochastic model, budget, failure policy, aggregation and confidence method. Retain each attempted call/run, invalid action, fallback and DNF. Paired deltas against fixed-plan and heuristic baselines; finish rate, race time/regret, stops, legal-action rate, cost/tokens and latency p50/p95. Average using declared scenario weights and report uncertainty; no fastest-seed selection, silent provider replacement or dropping failed calls. Token budgets require tokenizer/context disclosure and do not replace equal observation/tool access. Keep quality and cost/latency evaluations distinct.

Archive exact request/response, observation hashes, proposals/refusals/applied log, initial input/exogenous data, resolved artifacts, controller prompt/configuration/memory policy, provider revision or weights digest, quantization, inference engine/hardware, timeout/retry decisions, results and verification receipts. Full benchmark evidence has independent retention from disposable game replays. A third party must replay the archived actions offline under the pinned runtime; portable floating-point conformance follows #403 rather than an invented all-float equivalence claim.

Hosted identities have a controlled provenance; BYO model names remain self-declared unless independently attested. API revisions may disappear: action replay is guaranteed by retained artifacts, future provider inference is not. The public poll/submit contract in #417 never requires uploaded participant code or arbitrary callbacks. Infrastructure hosts approved inference adapters in isolated processes with bounded resources and no Authority secrets. #416 tests generic envelopes with a synthetic battery balance, without Racing imports or operational electricity-network claims.

## Deliverables and release boundary

Executable changes are separate tickets: model corrections; decision-surface campaign; manifest/eligibility contract; controller boundary; evaluation harness; public polling. Each needs source/schema vectors, positive/negative tests, a bounded scenario report and stated numerical guarantees. No Databricks job, cloud cost, DNS change or service activation is part of this planning update.

The game counterpart is `pitgun-game/docs/strategy/COMPETITIVE_ROADMAP_2026-09.md`; repository-specific ticket links are in [the delivery index](COMPETITIVE_BACKLOG_2026-09.md).

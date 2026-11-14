# Interactive Racing — one complete staging candidate

14 November 2026. Owner approved one complete staging delivery, rather than an
unranked hosted preview followed by ranked control. Physics/calibration is the
next separate release. Local implementation increments do not deploy services.

## Contract audit and first increment

Existing `RacingDriverInstructionAuthorizationV1` binds a driver-mode envelope to
the initial input. `RacingCompletedRunInputV1` binds the applied driver-instruction
timeline into a final input identity. Neither contract authorizes/replays the
experimental `control_prototype` Keep/tyre-service history. A signed driver-mode
attempt must never implicitly grant those new commands.

The provider-neutral schema catalog now adds experimental Engineer observation
v1/v2 and proposal v1. No new registry process is needed. The game embeds pinned
schema bytes; HTTP schema availability is not an execution dependency. v1 keeps
its exact input/prompt/cache identity. Opt-in native
`pitgun.racing-strategy-observation/v1` supplies the compound used on the completed
lap and remaining inclusive **declared own** stints; accepted services update later
observations only. It supplies no solved future performance, opponent plan or
host deadline. The game uses observation/prompt v2 only when those details exist.
Old binaries/configurations/observations stay unchanged on omission.

This fixes an information gap observed in the earlier live rationale: next-lap
medium did not establish the compound selected after stop38. The authored
regression now explicitly distinguishes completed-lap soft, next-lap medium and
post-stop hard. Structure and source facts are tested; that alone does not prove
model rationale truth or competitive strength.

## Ordered local integration work

1. **In progress:** immutable schema artifacts, semantic projection and held-out
   observations; bounded live-provider qualification; reference/model campaign
   on physically completing inputs. Keep negative/fallback/fuel cases.
2. **Open:** Racing-specific signed service-control envelope. Reuse the generic
   attempt lifecycle; bind initial input/catalog/model/runtime, exact control and
   observation policy, permitted player actions/tyres, limits and fallback rules.
   The server resolves/validates permissions; a client JSON field is not authority.
3. **Open:** completed service-control input/history, separate from legacy
   driver-instruction v1. Bind exact ordered applied history into the final run;
   replay regenerates observations/windows, validates proposals/receipts and
   rejects history omissions/substitution/reordering or effects outside permission.
4. **Open:** Authority/Verifier routes and admission/persistence, browser/Worker
   receipt/evidence and outbox/publication wiring. Preserve existing signatures,
   24h attempt expiry and immutable old runtime/catalog identities. Migrate no
   admitted run. No UI flag may bypass verification for leaderboard publication.
5. **Open:** authenticated hosted Engineer control route/relay, same-origin
   restrictions, fixed upstream/model, bounded payload/time/concurrency, durable
   cache and a deliberately shared quota budget. Existing hosted read-only
   debrief and local test ledgers must not accidentally double the public quota.
   Keys remain server-side. No arbitrary URL, tools or user-code execution.
6. **Open:** assemble matching committed native/browser/backend/infra artifacts;
   one local end-to-end rehearsal, then one staging promotion/owner recipe.

The exact service-control envelope and completion schemas need executable
producers/consumers before public promotion; do not turn a source-digest label or
hash-chained browser journal into a claim of remote attestation. The native domain
verdict governs actions. Provider identity/cost/timing claims remain distinct from
simulation replay; Verifier does not prove a given LLM authored a response.

## Staging acceptance

- Manual and Gemini paths use real legal actions, recorded fallback and exact
  replay, including an actual changed service, not only Keep.
- Durable authorization before execution; interrupted and completed reopening
  preserve attempt/input/action/result and settle rewards/publication once.
- Changed envelope/history/receipt/result, disallowed tyre, wrong observation,
  duplicate/late command and historical inference are rejected as appropriate.
- The ordinary verification/leaderboard path accepts only verified results and
  identifies the compatible execution/rules cohort; old results remain readable.
- Secrets stay off browser/logs/artifacts; protected hosted service remains bounded.
- Full-distance fuel failures (#414), GP memory (#430/game#315) and incomplete
  rival Compare remain visible limitations, not silently altered coefficients,
  reduced distances or fabricated finishes. A broken physical scenario cannot
  qualify as a successful full-GP benchmark.

No weather, hybrid energy, VSC or physical-parameter changes in this release.
The framework lifecycle stays domain-neutral; Racing owns stint/tyre semantics.

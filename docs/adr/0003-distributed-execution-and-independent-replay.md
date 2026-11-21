# ADR 0003: Distributed Execution and Independent Replay

- Status: Accepted direction; delivery is incremental, not all implemented
- Date: 2026-11-16
- Decision: owner authorization to progress the four distribution/verification axes
- Tracking: [framework434](https://github.com/loicbelec/pitgun/issues/434),
  [game325](https://github.com/loicbelec/pitgun-game/issues/325),
  [infra102](https://github.com/loicbelec/infra-vps/issues/102)

## Context and current guarantees

Pitgun distributes simulation to players' browsers. Certification currently
replays the **whole physical simulation**, including the recorded controller
actions, in the admitted, hash-pinned WASM runtime. The Verifier compares the
domain result and telemetry summary exactly, after validating authorization,
catalog, policy, canonical inputs, receipts and history. It does not merely
authenticate a client hash, nor attest every presentation point or the hardware
that performed the original run.

A compact evidence envelope saves transport and serialization, not the physical
replay itself. Current receipts are not cryptographic proofs of computation.
Replaying every distributed run centrally still incurs centralized physical
compute. The local compact experiment reduces waste but does not change this
trust model. Its local timings are not a hosted capacity promise.

## Decision: four boundaries

### 1. Interactive execution stays with the player

The browser Worker advances the admitted model, publishes bounded presentation
batches and persists recovery checkpoints. Rendering, playback rate and wall
clock do not affect the simulation outcome. The signed input, immutable runtime
and applied history remain the replay authority. Optimizations must preserve
observations, action effects, canonical evidence and client recovery digests.

No server round trip is required for each physical step. Controller inference
may be remote, but accepts only the model's bounded observation/action contract.

### 2. Exploration and certification are distinct product policies

Exploratory parameter sweeps and private experiments may be unranked. Only runs
admitted under a certification policy are eligible for a public verified result.
Authorization must precede those runs; selecting a result must never fabricate
an earlier signature or certify an unrecorded action history.

This is **not** a change to current Racing admission. Existing hosted practice
and race proofs, career rewards, anti-replay and publication guarantees remain
in force. A later application policy must specify which sessions are certified,
attempt/selection limits, budgets, seeds and ranking comparability. A public
benchmark must declare its selection rules; unbounded search followed by one
submission is not comparable to a fixed-budget challenge.

The reusable framework binds artifacts and replay semantics. The game or another
domain application owns eligibility, rewards and leaderboard policy. Do not put
Racing calendars, lap counts or career economy in generic scheduling code.

### 3. Replay consumes actions, never a fresh LLM response

Inference produces an untrusted structured proposal. Execution validates it and
records the resulting observation digest, action, timing and applied receipt.
Certification regenerates observations and validates/reapplies these recorded
proposals without contacting Gemini, a player endpoint or any other model.

That makes replay reproducible even when a provider is down or nondeterministic.
The model record documents provenance and configuration; it does not prove that
an external provider produced the action. Provider attestation or supervised
benchmark inference is a separate claim and future policy.

### 4. Replay placement is independent of the VPS

The control plane owns durable submissions, admission, deadlines, job leases,
verdict persistence and publication. Heavy replay may execute on another trusted
machine: another server, a workstation, a Mac mini or a cloud runner. No physical
engine API may depend on a VPS hostname or a particular queue transport.

The present `verify_service_with_replay` callback is sufficient to separate
authorization/evidence validation from replay execution. Keep this boundary
instead of introducing a universal distributed-compute framework before a second
domain proves the need. Runtime/catalog routing is operator-owned and immutable;
no client URL, path, uploaded code or mutable `latest` can select an executable.

## Trust boundary for remote execution

Current HTTP Verifier configuration uses shared **HMAC** Authority verification
material. Possessing that material also permits signing. Moving the entire
current Verifier therefore extends the high-trust zone; it is not an unprivileged
worker deployment. Asymmetric authorization would be a separate migration.

The preferred remote extension moves only the replay executor. It receives
validated, immutable execution inputs and history, **no Authority/HMAC key, model
API key or database write credential**. A registered executor can return
recomputed candidate evidence; the central verifier retains admission checks and
the final comparison/verdict boundary. Its computational integrity is trusted.
An authenticated worker can still lie if compromised: authentication and artifact
digests do not turn arbitrary volunteer machines into trusted verifiers.

Untrusted remote computing would require independent replicated execution,
auditing under a specified adversary model, or actual verifiable-computation
technology. None is silently supplied by a worker signature. No such mechanism
is introduced by this ADR.

## Durable execution envelope: requirements, not a published wire schema

For game325's later worker implementation, carry immutable references rather
than telemetry blobs in the queue. The minimal responsibilities are:

| Responsibility | Owner / binding |
| --- | --- |
| Job and execution ID, domain/adapter version | Server-owned durable job |
| Proof/input/history digest, model, runtime, bridge, pack, policy | Immutable admitted execution; validate retrieved bytes |
| First complete evidence receipt time | Server clock, bound durably to the proof before queueing |
| Worker identity, lease/attempt/fencing token | Authenticated private coordinator; never a browser assertion |
| CPU, memory, process and elapsed limits | Operator configuration; one heavy replay initially |
| Recomputed result/summary or operational failure | Bounded executor reply bound to job, proof and lease |
| Final verdict and publication | Control-plane transaction, idempotent projection |

A worker initiates outbound authenticated connections, preferably over a private
tunnel/VPN. Do not expose the current unauthenticated HTTP Verifier publicly.
Progress, timeout and lost lease are operational states, not proof rejection.
An unavailable executor leaves the result pending; the player can still finish
the locally calculated race and browse stored results.

Accept proofs atomically with a durable job and publication intent. Claim work
in short transactions, release row locks before replay, heartbeat bounded leases,
and refuse stale owners. Work may run more than once after failure; only one
terminal verdict/publication is committed. The first complete, timely receipt
must survive restarts and queue waits without extending the signed deadline.
The current private HTTP endpoint captures its own receipt time; it does **not**
yet accept a durable coordinator's earlier receipt time. A later authenticated
handoff is required before claiming that delayed jobs retain this guarantee.

## Incremental delivery and failure acceptance

1. Keep browser execution and current certification policy unchanged. Qualify
   compact replay under a new signed runtime; retain old runtime/adapter pairs
   and reject unknown pairs without loading code from requests.
2. Optimize browser materialization/copies with independent measurements. Keep
   client completion/recovery requirements; a Verifier-only compact reply is not
   a browser completion replacement.
3. Implement durable scheduling in the backend using PostgreSQL first, with
   explicit authenticated receipt-time handoff and bounded leases. Test: crash
   before/after acceptance, lost lease, stale completion, duplicated delivery,
   timely proof executed after deadline, late proof rejection, unavailable
   artifacts, timeout, restart and exactly one result projection.
4. Move an executor to a second trusted machine. Test wrong worker identity,
   wrong proof/runtime/pin, no credentials, interrupted connection, offline
   worker and recovery. Compare exact evidence to local execution. No remote
   rollout is authorized by a local routing change.
5. Add explicit exploration/certification policies for benchmark campaigns and
   a second domain. Document attempts, selection, resource accounting and model
   provenance separately from physical replay correctness.

Measure interactive compute, queue wait, replay wall/CPU/RSS, retries and
publication lag separately. Moving CPU away from the VPS changes capacity and
placement, not total certification work. Brokers, persistent Node pools, lower
sampling rates, reduced-physics verification and cryptographic computation
proofs remain separate decisions justified by measurements.

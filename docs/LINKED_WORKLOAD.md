# Statically Linked Workloads

Pitgun's first workload integration boundary is a Rust interface compiled into
the current native binary or WebAssembly artifact. It connects the generic
deterministic runtime to a domain Simulator without defining a universal
Solver or accepting code dynamically at runtime.

The public implementation lives in `pitgun-runtime`:

- `LinkedWorkload` binds a model identity, canonical input type, execution
  logic, domain output, and evidence type;
- `ExecutionContext` exposes the immutable deterministic run contract and its
  root seed;
- `WorkloadEvidence` projects the two domain-neutral digests recorded by an
  execution receipt;
- `execute_linked` validates contract bindings and orchestrates one execution.

## Execution Lifecycle

For one contract, adapter, and input, `execute_linked` performs these steps in
order:

1. require the adapter's exact model identity to match `contract.model`;
2. canonicalize the typed input with RFC 8785 and require its digest to match
   `contract.input.digest`;
3. calculate the logical `run_id` from the complete contract;
4. provide the immutable contract through `ExecutionContext`;
5. invoke the domain workload;
6. calculate the canonical output and telemetry-summary digests through the
   domain evidence implementation;
7. return the domain output, evidence, and calculated identities together.

Model and input mismatches fail before domain execution. Filesystem
persistence, execution identifiers, concrete runtime identity, receipt
creation, and Run Bundle layout remain application-adapter responsibilities.
Once those artifacts are loaded, their domain-neutral verification is owned by
`pitgun-runtime` as documented in
[Loaded Run Bundle Verification](RUN_BUNDLE_VERIFICATION.md).

## Domain Ownership

The interface does not prescribe how a domain computes its output. A domain
Simulator may call one or more domain Solvers, evolve state, emit events, and
produce telemetry before projecting its canonical evidence.

`RacingWorkload` is owned by `pitgun-racing-simulator`. The CLI depends on that
crate directly, constructs the deterministic contract, and invokes Racing
through `pitgun-runtime::execute_linked`; it does not call the Racing simulation
function directly. `pitgun-simulator::racing` remains a compatibility path.

Racing continues to own:

- `RunRaceInput` and `RaceOutput`;
- race orchestration, telemetry projection, and physical Solver invocation;
- `RacingRunEvidenceV1` and its canonical schema;
- Racing-specific execution failures.

## Compatibility and Scope

The linked interface is an internal Rust API, not a new wire-contract version.
The published deterministic evidence remains governed by the run contract,
RNG, canonical schemas, and golden vectors.

The first implementation deliberately excludes:

- a universal Solver or Simulator trait;
- dynamic loading of `.wasm` files;
- serialization of arbitrary workload implementations;
- network scheduling, tenancy, quotas, or sandbox capabilities;
- filesystem or CLI concerns inside `pitgun-runtime`.

A future external WASM component boundary requires a separate versioned ABI,
capability model, resource policy, artifact trust policy, and compatibility
decision.

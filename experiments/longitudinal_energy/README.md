# Native work–energy qualification V1

This is an **unregistered native physical candidate**, not a game/Authority runtime.
See [equations and limits](../../docs/physics/LONGITUDINAL_ENERGY_V1.md). Existing
catalogs and golden outputs are immutable. No cloud, Databricks, LLM, Docker service
or personal save is used. The probe resolves V9 career parameters once and calls
the Solver; it does not reproduce signed orchestration or browser costs.

## Reproduce

From a clean source commit with cached Rust dependencies:

```sh
cargo build --offline --release -p pitgun-racing-simulator \
  --example longitudinal_energy_probe --example physics_baseline_probe
PYTHONDONTWRITEBYTECODE=1 python3 experiments/longitudinal_energy/run_local.py \
  --probe target/release/examples/longitudinal_energy_probe \
  --legacy-probe target/release/examples/physics_baseline_probe
PYTHONDONTWRITEBYTECODE=1 python3 experiments/longitudinal_energy/run_local.py \
  --probe target/release/examples/longitudinal_energy_probe \
  --legacy-probe target/release/examples/physics_baseline_probe --check
```

Use the absolute target path if `CARGO_TARGET_DIR` is set. The runner rejects dirty
or untracked native sources, stale compiled source hashes, changed executables,
NaN/Infinity, a different frozen plan or a substituted catalog identity. SHA-256
records identify this experiment; they are not a reproducible-build attestation.
A report/documentation commit can follow the frozen source commit without changing
source hashes. `--check` repeats the campaign and never overwrites the archive.

## Fixed campaign and interpretation

[plan-v1.json](plan-v1.json) names ten exposed lot-0 reference cases, plus Spa with a
new seed reserved after the pilot, and a separate full-distance Barcelona modern
fuel-feasibility case. Spa was not used to tune the pilot but remains a single
synthetic configuration, not statistical held-out validation. Barcelona is related
to issue445, **not a reproduction of its missing original request/seed**. Only fuel
depletion is an allowed explicit failure for this additional feasibility case.

1. The original baseline probe must reproduce all ten archived **full canonical
   output summaries and digests** exactly. Historical goldens are never rewritten.
2. Each of the twelve pairs runs in four fresh processes, legacy/energy/energy/
   legacy. Repeated physical outcomes and their native result digests must match
   exactly for the same binary. Measurements alone may differ.
3. Successful candidate cars must meet the frozen per-segment/global energy, mass
   boundary, time, fuel and raw contact criteria. Aggregate energy closure alone
   is not independent evidence of physical accuracy.
4. Three fixed-geometry coarsenings and eight cooling/engine-air perturbations
   expose sensitivity. Removing geometric nodes is **not** an independently refined
   grid-convergence study. The smooth-driver option removes node-indexed control
   noise for these checks; separate analytic tests use 1/2/5/10m segments. Engine-air
   perturbations are not a complete weather model, and scaling conductance is not
   a new balanced HQ cooling upgrade.

The runner uses the existing local `wait4` helper for separate fresh-child CPU,
wall time and lifetime RSS. It runs sequentially, kills/reaps only its own child on
the 180s timeout and retains compact outcomes rather than full histories. The
solver still builds a native solution internally; compact reports do not reduce
its retained history. Core time excludes canonical hashing; complete process time
includes resolution, calculation, serialization/hashing and teardown. No browser,
network, WASM, VPS performance or bit parity is inferred.

The current empirical tire law, four fixed coupling passes, per-lap mass updates,
fixed driver envelopes and abstract pit speed reset remain declared approximations.
Do not label this report a validated digital twin, a Verifier admission or a model
accuracy/strategic superiority benchmark. The cooling sweep can expose a defect
without justifying coefficient fitting to these same cases.

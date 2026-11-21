# Physics lot 0 — local reference bench

This bench executes the **existing career physics** without changing equations,
coefficients, catalog bytes or model identities. It has no Authority, Verifier,
provider, registry or network client. Databricks, Delta and MLflow are not required.
Previous cloud evidence remains historical evidence for its own artifacts.

## Run

```sh
cargo build --offline --release -p pitgun-racing-simulator --example physics_baseline_probe
python3 -m unittest discover -s experiments/physics_foundation/tests -v
python3 experiments/physics_foundation/baseline_local.py --probe target/release/examples/physics_baseline_probe
python3 experiments/physics_foundation/baseline_local.py --probe target/release/examples/physics_baseline_probe --check
```

If `CARGO_TARGET_DIR` is set, pass the corresponding absolute probe path. Offline
builds require dependencies already cached; dependency acquisition is a separate
setup step. Measurements exclude compilation. No source of the solver/simulator
library is modified by this experiment.

The public [plan](plan-v1.json) contains ten cases and a fixed seed42. Each case
runs in two fresh native processes by default: four circuits, historical/modern/
hybrid-named resources, short sessions, full Monza53 and Monaco78, and a ten-car
five-lap grid. These are legal synthetic anchors, not an earned HQ career or
reproduction of a browser request. There is no coefficient sweep or optimization.
The Monaco case records a known fuel-depletion failure if encountered; a completed
case does not resolve every Monaco configuration. All failures remain explicit.

The optional `--jobs` is bounded1–4, default1 to limit memory pressure. The timeout
is per process, default180s, maximum600s. Each child owns temporary stdout/stderr
files; `wait4` records its own CPU and lifetime maximum RSS (macOS/Linux unit
conversion is explicit). It kills and reaps only that child on timeout. No global
process killing, background service or cloud compute is used.

## Identity and comparison

- The probe uses `LocalControlledRacingSession` with explicit
  `PowerUnitThermalV1`, the catalog's power-unit thermal profile, V9 defaults,
  service boundaries, an empty timeline and **no decision triggers**.
- This is the same physical branch selected by the current career bridge. The
  enclosing timeline catalog/model0.14 is not permission to run the fuel0.15
  branch or default offline0.9 profile instead. The physical core is0.11.
- The actual native executable is hashed and used as its runtime identity. The
  reference browser9af digest is not falsely attributed to this native binary.
- Reports pin the plan, binary, source files and complete local catalog JSON
  inventory; every request and canonical output digest is retained. Record the
  Git commit/toolchain alongside the report when publishing a new run.
  The executable embeds hashes of the reviewed core source files; the runner
  rejects it when those differ from the current tree. This guards against using
  a stale probe, without claiming a complete reproducible-build attestation.
- Repetitions compare the full deterministic summary, including output digest.
  `--check` requires the same identity and outcomes but excludes timings/RSS.
  It does not overwrite the archived report. A different binary may require a
  new baseline even with equivalent physical results.
- The1Hz/5Hz requested-rate pair compares native physical diagnostics separately
  from exported channels. **The existing branch hardcodes5Hz**, so identical
  export counts are reported rather than claimed as successful downsampling.

## Measurement boundary

`advance_ms` includes the existing native solver, simulator orchestration,
stream construction and final output materialization. It is **not pure solver
time**. Preparation and final summary/canonical hashing are separate. OS wall
time also includes startup, summary printing and teardown. Maximum RSS includes
native retained histories and hashing; it is not browser memory or a VPS budget.
Reported peaks from different processes are not added into a fictitious peak.

The report stores ranges and diagnostics, not the full telemetry history. The
probe still uses the current full-history path internally, so a compact report
does not remove native memory allocation. Last exported samples are not claimed
to be exact terminal states; native fuel/wear-after-service diagnostics retain
that distinction. No LLM is called and no result is signed, published or labelled
independently VERIFIED by this bench.

The baseline checks reproducibility and records known defects; it does not prove
physical accuracy, native/WASM bit parity, held-out generalization, statistical
robustness over seeds or strategic superiority. The fixed cases are all exposed
and therefore must not be relabelled unseen validation cases in later campaigns.
Future calibration and held-out sets must be designed separately.

CI runs the eight cheap runner/evidence checks. It does not replay this Mac's
frozen native binary/report on Linux and pretend to have established bit parity.

See [the lot-0 inventory and findings](../../docs/physics/FOUNDATION_2026-11-21.md).

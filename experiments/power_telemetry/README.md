# Local power telemetry V1 comparison

This is the first mechanical/energy slice after physics lot0. It adds a read-only,
versioned projection of the existing V3 native lap; no force, heat, wear, fuel,
lap time, model identity, historical channel or canonical evidence is replaced.
It is an offline candidate, not a new admitted browser runtime.

```sh
cargo build --offline --release -p pitgun-racing-simulator --example physics_baseline_probe
python3 experiments/power_telemetry/compare_local.py --probe target/release/examples/physics_baseline_probe
cargo test --offline -p pitgun-racing-solver --test physical_kernel power_v1_
```

With a custom `CARGO_TARGET_DIR`, pass the corresponding probe path. No dependency,
provider call, database, browser profile or hosted service is needed. Comparisons
use the frozen public lot0 plan and report. Those remain unchanged; to rerun the
original lot0 identity check, use its original commit/binary. Changing library
source legitimately changes the native binary identity.

The campaign runs each of ten cases three times sequentially: projection off,
then projection on twice. All three complete historical summaries and canonical
output hashes must equal the archived lot0 case. Both new power projections must
also match. The current executable and embedded core source hashes are checked;
no raw personal session or signed input is used. All execution has a180s bound.

For each native player lap, including the driving prefix before a pit service:

- Normalize delivered engine power by thermally available shaft power, with
  explicit drivetrain/shift limits. Reject material capacity mismatches; only
  documented floating-point roundoff may be clamped.
- Do not call this a measured pedal. No recorded pedal or actual brake command
  exists in this path, so both are `null`. Zero propulsion does not mean100%
  braking.
- Reconcile engine work, transmission loss, heat and fuel with the existing
  native diagnostics; per-lap thermal residual must be within0.01J.
- Name the unresolved low-speed force regularization separately. Do not turn
  it into a fabricated clutch heat source or silently change acceleration.
- Keep force/thermal-clock and final-trajectory-clock work separate. Their
  difference is evidence for a future physical coupling decision.
- Export1Hz or5Hz from the same native projection with bounded zero-order hold;
  the ledger is native and does not depend on either export cadence. This does
  not change the existing game exporter, which remains fixed5Hz.

The archived JSON contains per-lap ledgers, projection hashes and sample counts,
not the full power point history. CPU/RSS measures describe each native process;
projection-on and projection-off costs are not a browser or VPS speed claim.
Scope and criteria are in [the design/receipt](../../docs/physics/POWER_TELEMETRY_V1.md).

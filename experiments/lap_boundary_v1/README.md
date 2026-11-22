# Lap-boundary work and continuity — native V1

The current thermal V2 forecast uses the mathematical minimum of next-lap tire
grip. It does not replace the actual surface/carcass/wear states. That forecast
depends on fixed current-lap mass, compound, geometry and pinned next driver
controls; it is invariant during the current lap's tire/speed coupling iterations.

This increment caches that forecast **once per lap**. Energy V1's held-state
forecast remains recomputed because its endpoint reference changes. A new lap,
including a new compound after service, always starts with an empty cache.
No physical equation, coefficient, sampling grid or coupling tolerance changes.
Previous canonical results and errors must remain exact.

## Explicit offline diagnostic

`BoundaryDiagnosticRequestV1` owns the full resolved input, schema, forecast and
work mode. Hash this whole request for experiment identity. The ordinary session
constructor still selects minimum grip for V2 and returns no work counters.
No browser/Worker/catalog/workload/Authority selection enables the diagnostic.

Two modes compare the same physics: cached minimum grip and forced recomputation
of the same invariant. Complete result digests and accepted lap metrics must be
identical. A third mode, **end-state held**, is a diagnostic ablation of the
pessimism, not a proposed qualified lower bound on evolving future grip. It is
never silently selected as a fallback. Failed trajectories and accepted prefixes
are retained; no seam-speed trimming or rewriting of a committed lap is allowed.

Work counters describe attempted forecast evaluations/sweeps/cache hits, including
rejected numerical work. They are not accepted physical facts, future telemetry,
Engineer observations or journal events. Forecast temperature/wear coordinates
are explicitly separate from actual tire states.

The probe checks speed, engine temperature, surface/carcass temperature and wear
continuity using each previous committed post-service state. Its last300m
measurements are descriptive: real circuit curvature and the first upcoming
corner can legitimately require braking there. A difference against an unsafe
ablation is not proof of a recoverable lap-time gain.

## Frozen qualification

`plan-v1.json` is committed before measurement. Eight scenarios × three modes ×
two passes; cold/warm tires, early/classic/modern vehicles, three circuits, a tire
replacement and a full53-lap GP. Recheck56 earlier driver outcomes and22 frozen
historical/energy outcomes, preserving expected failures. Tests also compare
complete cached/recomputed results and unchanged pre-service continuation.

Then run the cost phase **after other native tests/campaigns finish**, serially:
three scenarios, seven repetitions, cached/recomputed order alternating. Clock
only `advance` calls; catalog resolution, constructor validation, metrics, final
digest and JSON serialization are excluded. Diagnostic bookkeeping is included.
Other Mac activity is not controlled. This is not browser/WASM/VPS qualification.

```sh
cargo build --offline --locked --release -p pitgun-racing-simulator \
  --example lap_boundary_probe --example thermal_driver_probe \
  --example longitudinal_energy_probe
python3 experiments/lap_boundary_v1/qualify_local.py screen \
  target/release/examples /tmp/pitgun-boundary-v1
# When unrelated native tests have ended, on the same clean source/binaries:
python3 experiments/lap_boundary_v1/qualify_local.py cost \
  target/release/examples /tmp/pitgun-boundary-v1
```

The cost phase refuses mismatched source/binary identity or unequal/failed floor
results. Preserve raw receipts privately and commit compact evidence afterwards.
Actual coupled look-ahead, its bounded horizon/cost, restored control continuity
and all-era competitive acceptance remain separate requirements. Current game,
personal browser saves/ports, deployed catalog/runtime and staging/prod stay as-is.

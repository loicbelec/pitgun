# Native energy cost and observation cadence V1

This slice preserves the equations, spatial nodes, four coupling passes, root
algorithm/tolerances, tire/engine states and event/control semantics of energy V1.
It removes repeated pure evaluations, prepares exact torque-knot values once per
unchanged engine, reuses computed root endpoints and avoids unused historical
contact work and intermediate copies. Existing game constructors still select the
historical model. No UI, signed runtime, old proof or hosted configuration changes.

## Three independent rates

- **Integration:** fixed spatial track nodes and solver passes; defines physical
  accuracy and most constrained-core cost. It is unchanged here.
- **Observation export:** periodic views of an already solved trajectory. The
  game currently hardcodes5Hz. The new native audit compares1/2/5Hz separately;
  changing its rate never changes a native result/digest.
- **Rendering:** browser animation cadence/interpolation; not measured or changed.

Lower observation rates reduce rows, serialization/hashing and transport/storage.
They do not remove already-computed root solves. Exact control/pit/incident events
must remain in their own journal, not be inferred from a downsampled signal. The
probe exports only named physical state fields using causal hold plus a terminal
point; it is **not** a game wire contract, an Engineer observation policy or a
recommended animation implementation. It has no command journal to downsample.

## Reproduce

Requires the exact frozen pre-optimization probe whose digest is recorded by
`../longitudinal_energy/results/local-energy-v1.json`. Preserve that executable
before rebuilding; historical reports are never overwritten. Then from clean
committed current sources and cached dependencies:

```sh
cargo build --offline --locked --release -p pitgun-racing-simulator \
  --example longitudinal_energy_probe --example physics_baseline_probe \
  --example energy_cost_probe
PYTHONDONTWRITEBYTECODE=1 python3 experiments/energy_cost/run_local.py \
  --before /path/to/frozen/longitudinal_energy_probe \
  --after target/release/examples/longitudinal_energy_probe \
  --legacy target/release/examples/physics_baseline_probe \
  --observations target/release/examples/energy_cost_probe
# Repeat with --check to compare deterministic evidence without archive writes.
```

The fixed plan requires exact complete physical outcomes against frozen energy V1
for twelve cases (including the same Barcelona fuel failure), all eleven prior
sensitivities, and ten historical default canonical results. Paired timing order
before/after/after/before uses fresh processes. Cadence order5/2/1/1/2/5 compares
four scenarios on the unchanged optimized physical solution. Binary/source/plan
identities are checked. OS wait4 records CPU/wall/RSS; two repetitions give an
order of magnitude, not statistical confidence across hardware. Compilation is
excluded. No browser/VPS/network bandwidth, graphics smoothness or new-model
native/WASM replay/admission is claimed.

An optional1s macOS `sample` of an owned frozen probe helped identify repeated
`SegmentContext::advance` / torque interpolation work. Such a short startup-window
profile is qualitative, not a trustworthy allocation of full-race percentages.
Raw process samples stay private; no personal process was profiled.

## Other paths to retain the physical model

1. Bound native history and materialize graphical details on demand; keep state
   checkpoints and verification evidence separate from retained display arrays.
2. Use compact numeric blocks/transferred buffers and rates by channel: position,
   engineering analysis and event journals need different delivery policies.
3. Continue incremental computation with a bounded playback buffer; masking a
   startup stall is different from reducing total CPU. A bounded worker pool may
   help on suitable hardware but must have a measured memory budget.
4. Consider an error-controlled integration mesh only as a **separate numerical
   experiment**. The same equations can use fewer nodes in smooth regions and more
   near braking/thermal changes, but this changes the numerical trajectory and
   needs convergence tests, event alignment and mesh-independent driver inputs.

None of these justifies deleting tire/thermal/energy phenomena or silently changing
an old runtime's observation rate/digest. Cost/fidelity and game readability gates
remain before integration into a new admitted runtime.

# Racing Model V3 compound-dependent tire degradation

This offline campaign introduces `pitgun.racing-v3-candidate@0.9.0`. It makes
tire wear depend on the selected compound, contact workload, and distance from
the compound's thermal operating window. It does not change the game, the
published Racing Catalog, Authority, Verifier, or any production model.

## Reduced-order law

For each resolved time step, Model V3 already computes a common aggregate
contact-patch workload:

```text
workload_power = available_force * velocity * utilization^2
```

Candidate `0.9.0` resolves wear as:

```text
thermal_deviation = (temperature - compound_optimum) / compound_sigma
thermal_multiplier = min(
  1 + thermal_gain * thermal_deviation^2,
  maximum_thermal_multiplier
)
compound_workload_multiplier = compound_wear_load_k / reference_wear_load_k

baseline_wear = compound_wear_per_second * dt
workload_wear = workload_power * dt
                / workload_energy_to_full_wear
                * compound_workload_multiplier
wear_delta = (baseline_wear + workload_wear) * thermal_multiplier
```

The tire resource therefore owns the compound's baseline wear, workload wear
coefficient, optimum temperature, thermal-window width, grip, and wear/grip
response. The V7 experiment profile owns only candidate-wide interpretation
coefficients. All are JSON inputs; no coefficient requires recompiling Rust.

`tire_contact.baseline_wear_per_s` remains in historical V3 profiles for V6
replay compatibility. Aggregate tire V2 deliberately uses the selected tire
resource's `wear_per_s` instead.

## Diagnostics

The probe records requested baseline and workload wear separately, minimum and
maximum thermal multipliers, and wear before and after service after every lap.
Those fields explain why a compound degraded; they are not hidden pace bonuses.

## Local campaign

Build the exact probe and execute the deterministic 236-point campaign:

```bash
cargo build --locked --release \
  -p pitgun-racing-simulator --example v3_validation_probe
python3 experiments/racing_v3_tire_degradation/validate_local.py --jobs 4
python3 experiments/racing_v3_tire_degradation/validate_local.py --jobs 4 --check
```

The campaign covers four representative circuits and four vehicle generations:

- 96 compound long runs across soft, medium, hard, 70 kg, and 100 kg fuel;
- 80 medium-to-soft one-stop windows;
- 24 compound/driver-control comparisons;
- 36 runtime screens over thermal gain and full-wear workload energy.

The canonical report is
[`results/local-tire-degradation-v1.json`](results/local-tire-degradation-v1.json).
Its 236 executions and 6,144 simulated laps establish that:

- accumulated wear is ordered soft > medium > hard in all 32 reviewed groups;
- compound pace crossovers exist in all 64 reviewed pairs;
- driver-control limits remain observable through contact workload;
- thermal and workload parameters are explorable without recompilation;
- the latest reviewed stop lap remains universally fastest.

The last result is intentionally a `REFINE` verdict. The structural connection
works, but the current coefficients are not calibrated: hard compounds are too
often globally preferable and later stops dominate. This report must not be
used as a production balance release.

## Databricks replay

The immutable Databricks manifest embeds the exact same 236 scenario/profile
pairs, their expected digests, and their expected experimental execution IDs.
The platform wheel embeds the same Rust `v3_validation_probe`; Python does not
reimplement the physical model.

After merge, attended execution is:

```bash
cd experiments/databricks
databricks bundle validate -t dev -p pitgun-free --strict
databricks bundle deploy -t dev -p pitgun-free
databricks bundle run v3_tire_degradation_job -t dev -p pitgun-free
```

The job is resumable through immutable natural keys. It persists raw
experimental results and normalized wear metrics to the existing governed
Delta tables, and logs the frozen manifest plus a compact report to MLflow.
Catalog promotion remains a separate human-reviewed decision.

Four parameter-screen baselines intentionally reuse the exact physical inputs
of a compound long run. Their analysis-role configuration IDs remain distinct,
while their experimental execution IDs are identical. This preserves both the
236-point analysis plan and honest content identity for equivalent executions.

## Compatibility boundary

Profile V7 opts into the new law and identity. Profiles V1–V6 retain their
previous identities and equations. A semantic V6 replay confirms that the
stored fuel-mass campaign differs only in the rebuilt probe binary digest, not
in model identity, results, or diagnostics.

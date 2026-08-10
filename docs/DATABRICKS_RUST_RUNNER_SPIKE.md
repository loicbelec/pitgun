# Databricks Rust Runner Spike

Status: selected packaging boundary implemented and proven on Free Edition for
issue #158.

## Decision

Pitgun packages the `aarch64-unknown-linux-gnu` CLI inside a platform-tagged
Python wheel. A small Python adapter copies that fixed package resource to
ephemeral storage, applies executable permissions, invokes it without a shell,
and returns its compact canonical JSON result.

Python performs orchestration only. It contains no Solver or Simulator equation
and cannot select another executable, URL, scenario, model, or data pack. The
Databricks job exposes only the unsigned 64-bit seed.

This boundary was selected because wheels are a supported Databricks job
library lifecycle, can carry package data, are uploaded by Declarative
Automation Bundles, and allow the native artifact to be content-addressed at
execution time.

## Build and identity

`adapter/build.sh` uses a pinned Rust Docker image under Linux/arm64, compiles
the existing `pitgun-cli` package with `--locked --release`, and creates a wheel
using a deterministic standard-library builder. The native binary itself is not
committed to Git.

Every adapter result records:

- the wheel version, including its source Git revision;
- the CLI-reported runner version;
- target `aarch64-unknown-linux-gnu`;
- SHA-256 digest of the exact executable bytes;
- SHA-256 digest of the canonical compact result bytes;
- host system and machine architecture;
- a separate process-start/version-probe duration;
- deterministic simulation execution duration.

Wall-clock measurements and host details are operational evidence. They remain
outside `pitgun.batch-run-result/v1` and therefore never alter `run_id`.

## Local proof

Build the wheel, install it in a clean Linux/arm64 Python container, and execute
the packaged fixture:

```bash
./adapter/build.sh

docker run --rm --platform linux/arm64 \
  --volume "$PWD/adapter/dist:/dist:ro" \
  --volume "$PWD/adapter/verify_local.py:/verify_local.py:ro" \
  python:3.12-slim \
  sh -c 'python -m pip install --no-index /dist/*.whl >/dev/null && python /verify_local.py'
```

The fixture pins `configuration_id`, `run_id`, and the digest of the exact
canonical result produced for seed 42. Databricks must reproduce all three.

A dependency-free probe executed before selecting the target reported
`Linux/aarch64` on Free Edition. An initial x86_64 wheel was rejected during
environment installation with `ERROR_WHEEL_INSTALLATION`, before any arbitrary
code ran. That evidence is why the bundle builds an ARM64 artifact rather than
assuming the architecture from a conventional cloud runtime.

## Databricks proof

The `runner_spike_job` is a serverless notebook task with the adapter wheel as
its only custom library:

```bash
databricks bundle deploy -t dev -p pitgun-free
databricks bundle run runner_spike_job -t dev -p pitgun-free --params seed=42
```

The notebook returns one `pitgun.databricks-runner-result/v1` document. The
runner and result digests allow downstream ingestion to reject an unexpected
artifact before writing a campaign row.

## Observed evidence

The seed-42 fixture produced the same
`sha256:19c045b5ccfb1ad789e8a3d74110efec919694883e05c5da996575e6986dfdef`
canonical result digest locally and on Databricks. Both executions also matched
the published configuration and run identities.

Measurements from the successful Free Edition run on 2026-08-10:

| Boundary | Local ARM64 container | Databricks serverless |
|---|---:|---:|
| Compute/environment setup | not applicable | 217,000 ms |
| Complete notebook execution | not applicable | 28,000 ms |
| Native version/startup probe | 1 ms | 17 ms |
| Deterministic Rust execution | 196 ms | 637 ms |

The first two Databricks values include platform scheduling, environment
installation, Python startup, and notebook overhead. They are operational cost,
not simulation evidence. The result document contains only the two measurements
made directly around native subprocesses.

## Options not selected

### Download a GitHub Release during every task

The existing `v0.1.0-alpha.1` release predates the machine-readable runner.
Runtime download would also add network availability to each simulation task.
A future immutable release can become the wheel build input, but not an online
runtime dependency.

### Execute a loose Workspace file

Workspace file synchronization does not express a Python library dependency and
does not guarantee executable permissions at its mounted path. Copying and
hashing would still be necessary. Embedding the file in a wheel provides a
supported installation and version lifecycle with a smaller exposed surface.

### Embed WASM in Python

The Racing Solver supports browser WASM, but `pitgun-cli` and its complete batch
boundary are native Rust. A Python WASM runtime would add another native package
and a second adapter contract without removing the need to package code. This
option remains useful for browser portability tests, not serverless execution.

### Compile Rust inside every Databricks task

This would require a toolchain, registry access, longer startup, and mutable
build inputs in the compute plane. Compilation belongs to artifact production;
campaign tasks execute immutable bytes only.

## Security boundary

- no shell is used to invoke Pitgun;
- no executable path, command, URL, model, data pack, or scenario is accepted
  from job parameters;
- the only variable input in the spike is the validated seed;
- execution has a fixed 120-second process timeout;
- successful runs must keep stderr empty and return the expected versioned JSON
  contract;
- the executable digest is calculated before every simulation.

# Pitgun Racing Quickstart

This quickstart reaches the complete deterministic boundary without an account,
VPS, container, registry, or external database:

**Scenario → Simulate → Observe → Persist → Replay → Verify**

The expected final line for the versioned seed-42 workload is:

```text
VERIFIED sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a
```

## Run from the workspace

Prerequisites: Git and the stable Rust toolchain from
[rustup](https://rustup.rs).

```bash
git clone https://github.com/loicbelec/pitgun.git
cd pitgun
cargo run --locked -p pitgun-cli -- \
  demo racing --seed 42 --output ./pitgun-quickstart-run
```

The first invocation includes Rust dependency download and compilation. The
simulation, persistence, replay, and verification themselves require no network
service. On subsequent invocations, Pitgun validates and reuses the immutable
bundle instead of overwriting it.

## Run a prebuilt binary

Version `v0.1.0-alpha.1` initially supports:

- Apple Silicon macOS: `aarch64-apple-darwin`;
- Intel macOS: `x86_64-apple-darwin`;
- 64-bit Intel/AMD Linux with glibc: `x86_64-unknown-linux-gnu`.

Download the archive and `SHA256SUMS` from the
[GitHub Release](https://github.com/loicbelec/pitgun/releases/tag/v0.1.0-alpha.1).

For Apple Silicon macOS:

```bash
version=v0.1.0-alpha.1
target=aarch64-apple-darwin
archive="pitgun-${version}-${target}.tar.gz"

curl -fLO "https://github.com/loicbelec/pitgun/releases/download/${version}/${archive}"
curl -fLO "https://github.com/loicbelec/pitgun/releases/download/${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 --check
tar -xzf "${archive}"

"./pitgun-${version}-${target}/pitgun" --version
"./pitgun-${version}-${target}/pitgun" \
  demo racing --seed 42 --output ./pitgun-quickstart-run
```

For x86-64 Linux, use `target=x86_64-unknown-linux-gnu` and replace the checksum
command with:

```bash
grep " ${archive}$" SHA256SUMS | sha256sum --check
```

For Intel macOS, use `target=x86_64-apple-darwin` with the macOS commands above.

The prebuilt archives are unsigned alpha artifacts. Always validate the
published checksum before running a binary. Publishing `pitgun-cli` to
crates.io remains intentionally out of scope until its internal dependencies
and public APIs are ready.

## Understand the Run Bundle

The selected output directory contains the portable Run Bundle V1:

| File | Role |
|---|---|
| `manifest.json` | Versioned layout, run identity, and artifact digests |
| `scenario.json` | Canonical Racing scenario |
| `contract.json` | Deterministic execution contract and seed |
| `output.json` | Canonical simulation result |
| `telemetry.jsonl` | Ordered typed telemetry frames |
| `telemetry-summary.json` | Recalculated telemetry evidence |
| `metrics.json` | Derived metrics, including observed maximum speed |
| `receipt.json` | Concrete execution receipt |

Verify the committed bundle again in a fresh process:

```bash
cargo run --locked -p pitgun-cli -- replay ./pitgun-quickstart-run
```

When using an extracted release, replace `cargo run --locked -p pitgun-cli --`
with the binary path shown in the installation commands.

## Provoke a safe verification failure

Keep the verified bundle intact and mutate a copy:

```bash
cp -R ./pitgun-quickstart-run ./pitgun-tampered-run
perl -0pi -e 's/"total_time_ms":85310/"total_time_ms":1/' \
  ./pitgun-tampered-run/output.json
cargo run --locked -p pitgun-cli -- replay ./pitgun-tampered-run
```

Pitgun exits with code `50`, identifies the `output.json` digest mismatch, and
does not print `VERIFIED`. Replay the original directory to confirm it still
passes. This demonstrates artifact-integrity verification; it does not prove
that the physical model itself is an accurate representation of reality.

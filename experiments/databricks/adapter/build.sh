#!/usr/bin/env bash
set -euo pipefail

adapter_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
framework_root="$(cd "${adapter_root}/../../.." && pwd)"

mkdir -p "${adapter_root}/build"

docker run --rm \
  --platform linux/arm64 \
  --env CARGO_TARGET_DIR=/pitgun-target \
  --env HOST_UID="$(id -u)" \
  --env HOST_GID="$(id -g)" \
  --volume pitgun-cargo-registry-aarch64:/usr/local/cargo/registry \
  --volume pitgun-cargo-target-aarch64:/pitgun-target \
  --volume "${framework_root}:/workspace" \
  --workdir /workspace \
  rust:1.90-bookworm@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f \
  sh -c 'cargo build --locked --release -p pitgun-cli && cp "$CARGO_TARGET_DIR/release/pitgun" /workspace/experiments/databricks/adapter/build/pitgun && chown "$HOST_UID:$HOST_GID" /workspace/experiments/databricks/adapter/build/pitgun'

python3 "${adapter_root}/build_wheel.py"

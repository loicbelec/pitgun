#!/usr/bin/env bash
set -euo pipefail

: "${PITGUN_SIGNING_SECRET:?PITGUN_SIGNING_SECRET is required}"
: "${PITGUN_TELEMETRY_URL:=http://127.0.0.1:8080/beacon}"

export PITGUN_SIGNING_SECRET

cargo run -p pitgun-source-physics --bin physics_http_emitter -- \
  --telemetryd-url "${PITGUN_TELEMETRY_URL}" \
  --contract-json tmp/sim_contract.json \
  --verify-signature

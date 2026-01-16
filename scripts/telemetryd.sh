#!/usr/bin/env bash
set -euo pipefail

: "${PITGUN_TELEMETRY_BIND:=127.0.0.1:8080}"
: "${PITGUN_TELEMETRY_DATA_DIR:=./data/ingest}"

export PITGUN_TELEMETRY_BIND
export PITGUN_TELEMETRY_DATA_DIR

cargo run -p pitgun-telemetryd --release

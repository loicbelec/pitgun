#!/usr/bin/env bash
set -euo pipefail

: "${PITGUN_SIGNING_SECRET:?PITGUN_SIGNING_SECRET is required}"
: "${PITGUN_CONTRACT_TTL_MS:=600000}"
: "${PITGUN_CONFIGD_BIND:=127.0.0.1:8081}"

export PITGUN_SIGNING_SECRET
export PITGUN_CONTRACT_TTL_MS
export PITGUN_CONFIGD_BIND

cargo run -p pitgun-configd --release

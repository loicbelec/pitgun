#!/usr/bin/env bash
set -euo pipefail

: "${PITGUN_CONFIGD_URL:=http://127.0.0.1:8081}"

mkdir -p tmp

curl -sS -X POST "${PITGUN_CONFIGD_URL}/v1/requests/game" \
  -H 'content-type: application/json' \
  -d @examples/sim_request.json \
  > tmp/sim_contract.json

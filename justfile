set dotenv-load := true

telemetryd:
    PITGUN_TELEMETRY_BIND="${PITGUN_TELEMETRY_BIND:-127.0.0.1:8080}" \
    PITGUN_TELEMETRY_DATA_DIR="${PITGUN_TELEMETRY_DATA_DIR:-./data/ingest}" \
    cargo run -p pitgun-telemetryd --release

configd:
    PITGUN_SIGNING_SECRET="${PITGUN_SIGNING_SECRET:?PITGUN_SIGNING_SECRET is required}" \
    PITGUN_CONTRACT_TTL_MS="${PITGUN_CONTRACT_TTL_MS:-600000}" \
    PITGUN_CONFIGD_BIND="${PITGUN_CONFIGD_BIND:-127.0.0.1:8081}" \
    cargo run -p pitgun-configd --release

contract:
    curl -sS -X POST "${PITGUN_CONFIGD_URL:-http://127.0.0.1:8081}/v1/requests/game" \
      -H 'content-type: application/json' \
      -d @examples/sim_request.json \
      > examples/sim_contract.json

emit-http:
    PITGUN_SIGNING_SECRET="${PITGUN_SIGNING_SECRET:?PITGUN_SIGNING_SECRET is required}" \
    cargo run -p pitgun-source-physics --bin physics_http_emitter -- \
      --telemetryd-url "${PITGUN_TELEMETRY_URL:-http://127.0.0.1:8080/beacon}" \
      --contract-json examples/sim_contract.json \
      --verify-signature

emit-ws:
    PITGUN_SIGNING_SECRET="${PITGUN_SIGNING_SECRET:?PITGUN_SIGNING_SECRET is required}" \
    cargo run -p pitgun-source-physics --bin physics_http_emitter -- \
      --ws-url "${PITGUN_TELEMETRY_WS_URL:-ws://127.0.0.1:8080/ws}" \
      --contract-json examples/sim_contract.json \
      --verify-signature

e2e-http: contract emit-http

e2e-ws: contract emit-ws

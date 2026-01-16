# pitgun-configd

## Game simulation request endpoint

`POST /v1/requests/game`

The service normalizes the six tuning parameters, validates basic bounds, and
returns a signed `GameSimulationContractV1` containing the request plus
issued/expires metadata. The signature is computed over a canonical binary
representation of the contract payload to ensure stable verification.

TTL is controlled by `PITGUN_CONTRACT_TTL_MS` (default: `600000`).

Example:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests/game \
  -H 'content-type: application/json' \
  -d '{
    "track_id": "demo-oval",
    "hz": 60.0,
    "tuning": {
      "aero_points": 10,
      "chassis_points": 10,
      "engine_points": 10,
      "cooling_points": 10,
      "downforce_slider": 0.5,
      "gear_ratio_slider": 0.5
    },
    "seed": 1,
    "engine_version": "0.1.0"
  }'
```

Example response:

```json
{
  "request": {
    "track_id": "demo-oval",
    "hz": 60.0,
    "tuning": {
      "aero_points": 10,
      "chassis_points": 10,
      "engine_points": 10,
      "cooling_points": 10,
      "downforce_slider": 0.5,
      "gear_ratio_slider": 0.5
    },
    "seed": 1,
    "engine_version": "0.1.0"
  },
  "issued_at_ms": 1710000000123,
  "expires_at_ms": 1710000600123,
  "nonce": "2c8a1b96-7f53-4d0e-9b3d-9dce255a1c32",
  "signature": "hex_hmac_sha256"
}
```

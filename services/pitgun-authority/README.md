# pitgun-authority

## Simulation contract endpoint

`POST /v1/contracts/simulation`

The service loads `policies/gametuning.v1.yaml` (override with `PITGUN_TUNING_POLICY_PATH`),
canonicalizes the tuning request, validates derived constraints, and returns a
signed `SimulationContractV1` payload. The signature is computed over the JSON
serialization of the `contract` object.

Example:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/contracts/simulation \
  -H 'content-type: application/json' \
  -d '{
    "era": 3,
    "category_levels": {"budget_lvl": 100},
    "owned_upgrades": [],
    "parameters": {
      "gameplay": {
        "aero_points": 25.0,
        "chassis_points": 25.0,
        "cooling_points": 25.0,
        "engine_points": 25.0,
        "downforce_slider": 0.5,
        "gear_ratio_slider": 0.5
      }
    }
  }'
```

Example response:

```json
{
  "contract": {
    "version": "SimulationContractV1",
    "issued_at_ms": 1710000000000,
    "expires_at_ms": 1710000300000,
    "era": 3,
    "category_levels": {
      "budget_lvl": 100
    },
    "owned_upgrades": [],
    "parameters": {
      "gameplay": {
        "aero_points": 25.0,
        "chassis_points": 25.0,
        "cooling_points": 25.0,
        "engine_points": 25.0,
        "downforce_slider": 0.5,
        "gear_ratio_slider": 0.5
      }
    },
    "derived_constraints": ["gameplay_budget_cap"],
    "policy_hash": "hex_sha256"
  },
  "signature": "hex_hmac_sha256"
}
```

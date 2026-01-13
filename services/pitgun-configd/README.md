# pitgun-configd

## Simulation contract endpoint

`POST /v1/contracts/simulation`

The service loads `policies/tuning.v1.yaml` (override with `PITGUN_TUNING_POLICY_PATH`),
canonicalizes the tuning request, validates derived constraints, and returns a
signed `SimulationContractV1` payload. The signature is computed over the JSON
serialization of the `contract` object.

Example:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/contracts/simulation \
  -H 'content-type: application/json' \
  -d '{
    "era": 3,
    "category_levels": {"mech_lvl": 5, "testing_lvl": 10, "manufacturing_lvl": 15, "it_systems_lvl": 20},
    "owned_upgrades": ["e2_turbocharger", "e2_hybrid_sys"],
    "parameters": {
      "aero": {"front_wing_angle": 18.0, "rear_wing_angle": 22.0},
      "powertrain": {"turbo_boost_pressure": 1.6}
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
      "mech_lvl": 5,
      "testing_lvl": 10,
      "manufacturing_lvl": 15,
      "it_systems_lvl": 20
    },
    "owned_upgrades": ["e2_turbocharger", "e2_hybrid_sys"],
    "parameters": {
      "aero": { "front_wing_angle": 18.0, "rear_wing_angle": 22.0 },
      "powertrain": { "turbo_boost_pressure": 1.6 }
    },
    "derived_constraints": ["wing_balance", "turbo_lean_protection", "active_suspension_energy"],
    "policy_hash": "hex_sha256"
  },
  "signature": "hex_hmac_sha256"
}
```

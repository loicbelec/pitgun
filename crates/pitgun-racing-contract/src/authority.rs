use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Racing setup contract signed by the authority service.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationContractV1 {
    pub version: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub era: u32,
    pub category_levels: BTreeMap<String, i64>,
    pub owned_upgrades: Vec<String>,
    pub parameters: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_constraints: Option<Vec<String>>,
    pub policy_hash: String,
}

/// Authority response containing a Racing setup contract and its signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedSimulationContractV1 {
    pub contract: SimulationContractV1,
    pub signature: String,
}

impl SimulationContractV1 {
    /// Returns the historical byte representation protected by the authority
    /// HMAC. This is a published compatibility boundary, not RFC 8785 JSON.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut canonical = self.clone();
        canonical.owned_upgrades.sort();
        canonical.parameters = canonicalize_json(canonical.parameters);
        serde_json::to_vec(&canonical)
    }
}

fn canonicalize_json(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut items: Vec<(String, JsonValue)> = map.into_iter().collect();
            items.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut ordered = serde_json::Map::new();
            for (key, value) in items {
                ordered.insert(key, canonicalize_json(value));
            }
            JsonValue::Object(ordered)
        }
        JsonValue::Array(values) => {
            JsonValue::Array(values.into_iter().map(canonicalize_json).collect())
        }
        other => other,
    }
}

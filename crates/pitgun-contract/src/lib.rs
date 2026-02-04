pub mod frame;
pub mod registry;
pub mod source;

pub use frame::{
    Event, EventData, EventId, EventSeverity, ParameterId, Sample, SampleValue, SessionId,
    SignalQuality, TelemetryFrame, TelemetryFrameBuilder,
};
pub use registry::{
    AccessLevel, Conversion, DataType, Parameter, ParameterRegistry, Range, RegistryError,
    ValidationResult,
};
pub use source::{
    SourceConfig, SourceError, SourceMetadata, SourceResult, SourceState, SourceStats,
    SourceType, TelemetrySource,
};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

pub const MODEL_VERSION_V1: &str = "perf-eq-v1";
pub const SCHEMA_VERSION_V1: &str = "telemetry-schema-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigRequestV1 {
    pub subject: String,
    pub client_id: String,
    pub game_version: String,
    pub upgrades: Vec<String>,
    pub tuning: Vec<TuningParam>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TuningParam {
    pub key: String,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CanonicalConfigV1 {
    pub subject: String,
    pub client_id: String,
    pub game_version: String,
    pub upgrades: Vec<String>,
    pub tuning: Vec<TuningParam>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractLimitsV1 {
    pub max_hz: u32,
    pub max_duration_s: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigContractPayloadV1 {
    pub model_version: String,
    pub schema_version: String,
    pub wire_format_id: String,
    pub telemetry_endpoint: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub seed: u64,
    pub limits: ContractLimitsV1,
    pub config: CanonicalConfigV1,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigContractV1 {
    pub model_version: String,
    pub schema_version: String,
    pub wire_format_id: String,
    pub telemetry_endpoint: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub seed: u64,
    pub limits: ContractLimitsV1,
    pub config: CanonicalConfigV1,
    pub signature: String,
}

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedSimulationContractV1 {
    pub contract: SimulationContractV1,
    pub signature: String,
}

impl ConfigContractPayloadV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

impl ConfigContractV1 {
    pub fn payload(&self) -> ConfigContractPayloadV1 {
        ConfigContractPayloadV1 {
            model_version: self.model_version.clone(),
            schema_version: self.schema_version.clone(),
            wire_format_id: self.wire_format_id.clone(),
            telemetry_endpoint: self.telemetry_endpoint.clone(),
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
            seed: self.seed,
            limits: self.limits.clone(),
            config: self.config.clone(),
        }
    }
}

impl SimulationContractV1 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_signing::SigningKey;
    use serde_json::json;

    #[test]
    fn signs_and_verifies_config_contract_payload() {
        let payload = ConfigContractPayloadV1 {
            model_version: MODEL_VERSION_V1.to_string(),
            schema_version: SCHEMA_VERSION_V1.to_string(),
            wire_format_id: "session-envelope-json-v1".to_string(),
            telemetry_endpoint: "ws://127.0.0.1:8080/ws".to_string(),
            issued_at_ms: 1_710_000_000_000,
            expires_at_ms: 1_710_000_600_000,
            seed: 42,
            limits: ContractLimitsV1 {
                max_hz: 60,
                max_duration_s: 300,
            },
            config: CanonicalConfigV1 {
                subject: "driver-1".to_string(),
                client_id: "client-1".to_string(),
                game_version: "1.0.0".to_string(),
                upgrades: vec!["aero-kit".to_string()],
                tuning: vec![
                    TuningParam {
                        key: "brake_bias".to_string(),
                        value: 0.5,
                    },
                    TuningParam {
                        key: "steering_gain".to_string(),
                        value: 1.0,
                    },
                    TuningParam {
                        key: "throttle_gain".to_string(),
                        value: 1.0,
                    },
                ],
            },
        };

        let bytes = payload.signing_bytes().expect("payload should serialize");
        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid");
        let signature = key.sign(&bytes);

        assert!(key.verify(&bytes, &signature));
    }

    #[test]
    fn simulation_contract_signing_is_deterministic() {
        let base = SimulationContractV1 {
            version: "SimulationContractV1".to_string(),
            issued_at_ms: 1_710_000_000_000,
            expires_at_ms: 1_710_000_600_000,
            era: 3,
            category_levels: BTreeMap::from([
                ("mech_lvl".to_string(), 5),
                ("testing_lvl".to_string(), 10),
            ]),
            owned_upgrades: vec!["e2_hybrid_sys".to_string(), "e2_turbocharger".to_string()],
            parameters: json!({
                "powertrain": {
                    "turbo_boost_pressure": 1.6,
                    "fuel_mixture": "standard"
                },
                "aero": {
                    "rear_wing_angle": 22.0,
                    "front_wing_angle": 18.0
                }
            }),
            derived_constraints: None,
            policy_hash: "deadbeef".to_string(),
        };

        let reordered = SimulationContractV1 {
            owned_upgrades: vec!["e2_turbocharger".to_string(), "e2_hybrid_sys".to_string()],
            parameters: json!({
                "aero": {
                    "front_wing_angle": 18.0,
                    "rear_wing_angle": 22.0
                },
                "powertrain": {
                    "fuel_mixture": "standard",
                    "turbo_boost_pressure": 1.6
                }
            }),
            ..base.clone()
        };

        let base_bytes = base.signing_bytes().expect("payload should serialize");
        let reordered_bytes = reordered
            .signing_bytes()
            .expect("payload should serialize");
        assert_eq!(base_bytes, reordered_bytes);

        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid");
        let signature = key.sign(&base_bytes);
        assert!(key.verify(&reordered_bytes, &signature));
    }
}

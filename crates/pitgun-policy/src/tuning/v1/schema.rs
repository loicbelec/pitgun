use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TuningPolicyV1 {
    pub version: String,
    pub meta: Option<TuningMeta>,
    pub parameters: BTreeMap<String, BTreeMap<String, ParameterSpecV1>>,
    pub derived_constraints: Option<Vec<DerivedConstraint>>,
    pub telemetry_schema_hint: Option<TelemetrySchemaHint>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TuningMeta {
    pub description: Option<String>,
    pub determinism: Option<DeterminismMeta>,
    pub signing: Option<SigningMeta>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeterminismMeta {
    pub numeric: Option<String>,
    pub tick_hz: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningMeta {
    pub contract_type: Option<String>,
    pub fields_to_sign: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetrySchemaHint {
    pub channels: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloatRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type", deny_unknown_fields)]
pub enum ParameterSpecV1 {
    Float {
        unit: Option<String>,
        range: FloatRange,
        default: f64,
        unlock: Option<String>,
        effects: Option<BTreeMap<String, String>>,
        meaning: Option<String>,
    },
    Enum {
        values: Vec<String>,
        default: String,
        unlock: Option<String>,
        meaning: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedConstraint {
    pub name: String,
    pub rule: String,
    pub error_msg: String,
}

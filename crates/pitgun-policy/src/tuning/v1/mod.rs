use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::Value as JsonValue;

pub mod canonicalize;
pub mod error;
pub mod expr;
pub mod schema;
pub mod validate;

pub use error::PolicyError;
pub use schema::{
    DerivedConstraint, DeterminismMeta, FloatRange, ParameterSpecV1, SigningMeta, TelemetrySchemaHint,
    TuningMeta, TuningPolicyV1,
};

pub const TUNING_POLICY_V1_VERSION: &str = "tuning.v1";

#[derive(Clone, Debug)]
pub struct TuningEvalContext {
    pub era: u32,
    pub category_levels: BTreeMap<String, i64>,
    pub owned_upgrades: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct PlayerTuningRequest {
    pub parameters: JsonValue,
}

#[derive(Clone, Debug)]
pub struct CanonicalTuningParameters {
    pub parameters: JsonValue,
}

/// Parse a tuning.v1 policy from a YAML string.
pub fn load_tuning_v1_from_str(yaml: &str) -> Result<TuningPolicyV1, PolicyError> {
    let policy: TuningPolicyV1 = serde_yaml::from_str(yaml).map_err(PolicyError::InvalidYaml)?;
    if policy.version != TUNING_POLICY_V1_VERSION {
        return Err(PolicyError::UnsupportedVersion(policy.version));
    }
    Ok(policy)
}

/// Read a tuning.v1 policy from disk and parse it.
pub fn load_tuning_v1_from_path(path: impl AsRef<Path>) -> Result<TuningPolicyV1, PolicyError> {
    let contents = fs::read_to_string(path).map_err(PolicyError::Io)?;
    load_tuning_v1_from_str(&contents)
}

impl TuningPolicyV1 {
    pub fn validate_static(&self) -> Result<(), PolicyError> {
        validate::validate_static(self)
    }

    pub fn canonicalize(
        &self,
        ctx: &TuningEvalContext,
        req: &PlayerTuningRequest,
    ) -> Result<CanonicalTuningParameters, PolicyError> {
        canonicalize::canonicalize(self, ctx, req)
    }

    pub fn validate_constraints(
        &self,
        ctx: &TuningEvalContext,
        canonical: &CanonicalTuningParameters,
    ) -> Result<(), PolicyError> {
        canonicalize::validate_constraints(self, ctx, canonical)
    }
}

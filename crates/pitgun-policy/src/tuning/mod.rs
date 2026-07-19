use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub mod v1;

pub use v1::{
    CanonicalTuningParameters, ParameterSpecV1, PlayerTuningRequest, TuningEvalContext,
    TuningPolicyV1,
};
pub use v1::{
    DerivedConstraint, DeterminismMeta, FloatRange, SigningMeta, TelemetrySchemaHint, TuningMeta,
};
pub use v1::{PolicyError, load_tuning_v1_from_path, load_tuning_v1_from_str};

pub const POLICY_VERSION_V1: &str = "tuning-policy-v1";
pub const POLICY_PATH_ENV: &str = "PITGUN_TUNING_POLICY_PATH";
pub const POLICY_STRICT_ENV: &str = "PITGUN_POLICY_STRICT";

/// Generic key-value parameter accepted by the policy normalization engine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TuningParam {
    pub key: String,
    pub value: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TuningPolicy {
    pub version: String,
    pub parameters: HashMap<String, ParameterSpec>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum ParameterSpec {
    Float {
        min: f64,
        max: f64,
        default: Option<f64>,
    },
    Int {
        min: i64,
        max: i64,
        default: Option<i64>,
    },
    Enum {
        values: Vec<String>,
        default: Option<String>,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum StrictMode {
    Strict,
    Permissive,
}

impl TuningPolicy {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PolicyError> {
        let contents = fs::read_to_string(path).map_err(PolicyError::Io)?;
        let policy: TuningPolicy =
            serde_yaml::from_str(&contents).map_err(PolicyError::InvalidYaml)?;

        if policy.version != POLICY_VERSION_V1 {
            return Err(PolicyError::UnsupportedVersion(policy.version));
        }
        if policy.parameters.is_empty() {
            return Err(PolicyError::MissingParameters);
        }

        Ok(policy)
    }

    pub fn normalize_tuning(
        &self,
        input: Vec<TuningParam>,
        mode: StrictMode,
    ) -> Result<Vec<TuningParam>, PolicyError> {
        let mut normalized = HashMap::new();

        // Duplicate keys are deduped with last value winning.
        for param in input {
            let key = param.key.trim().to_string();
            if key.is_empty() {
                continue;
            }

            let Some(spec) = self.parameters.get(&key) else {
                match mode {
                    StrictMode::Strict => return Err(PolicyError::UnknownKey(key)),
                    StrictMode::Permissive => continue,
                }
            };

            let value = normalize_value(&key, param.value, spec)?;
            normalized.insert(key, value);
        }

        let mut output: Vec<TuningParam> = normalized
            .into_iter()
            .map(|(key, value)| TuningParam { key, value })
            .collect();
        output.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(output)
    }
}

pub fn default_policy_path() -> PathBuf {
    std::env::var(POLICY_PATH_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("policies/gametuning.v1.yaml"))
}

pub fn strict_mode_from_env() -> StrictMode {
    match std::env::var(POLICY_STRICT_ENV) {
        Ok(value) => {
            let raw = value.trim().to_ascii_lowercase();
            if raw == "0" || raw == "false" || raw == "no" {
                StrictMode::Permissive
            } else {
                StrictMode::Strict
            }
        }
        Err(_) => StrictMode::Strict,
    }
}

fn normalize_value(key: &str, value: f64, spec: &ParameterSpec) -> Result<f64, PolicyError> {
    if !value.is_finite() {
        return Err(PolicyError::InvalidValue {
            key: key.to_string(),
            reason: "must be finite".to_string(),
        });
    }

    match spec {
        ParameterSpec::Float { min, max, .. } => Ok(value.clamp(*min, *max)),
        ParameterSpec::Int { min, max, .. } => {
            let rounded = value.round();
            if rounded < *min as f64 || rounded > *max as f64 {
                let clamped = rounded.clamp(*min as f64, *max as f64);
                Ok(clamped)
            } else {
                Ok(rounded)
            }
        }
        ParameterSpec::Enum { values, .. } => {
            let as_int = value.round();
            if as_int < 0.0 || as_int as usize >= values.len() {
                return Err(PolicyError::InvalidValue {
                    key: key.to_string(),
                    reason: "enum index out of range".to_string(),
                });
            }
            Ok(as_int)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_policy() -> TuningPolicy {
        let mut parameters = HashMap::new();
        parameters.insert(
            "steering_gain".to_string(),
            ParameterSpec::Float {
                min: 0.5,
                max: 3.0,
                default: Some(1.0),
            },
        );
        parameters.insert(
            "brake_bias".to_string(),
            ParameterSpec::Float {
                min: 0.0,
                max: 1.0,
                default: None,
            },
        );

        TuningPolicy {
            version: POLICY_VERSION_V1.to_string(),
            parameters,
        }
    }

    #[test]
    fn unknown_key_errors_in_strict_mode() {
        let policy = sample_policy();
        let input = vec![TuningParam {
            key: "unknown".to_string(),
            value: 1.0,
        }];

        let err = policy
            .normalize_tuning(input, StrictMode::Strict)
            .unwrap_err();

        match err {
            PolicyError::UnknownKey(key) => assert_eq!(key, "unknown"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn unknown_key_ignored_in_permissive_mode() {
        let policy = sample_policy();
        let input = vec![TuningParam {
            key: "unknown".to_string(),
            value: 1.0,
        }];

        let output = policy
            .normalize_tuning(input, StrictMode::Permissive)
            .expect("should ignore unknown key");

        assert!(output.is_empty());
    }

    #[test]
    fn clamps_numeric_values() {
        let policy = sample_policy();
        let input = vec![TuningParam {
            key: "steering_gain".to_string(),
            value: 10.0,
        }];

        let output = policy
            .normalize_tuning(input, StrictMode::Strict)
            .expect("should clamp values");

        assert_eq!(output.len(), 1);
        assert_eq!(output[0].value, 3.0);
    }

    #[test]
    fn sorts_keys() {
        let policy = sample_policy();
        let input = vec![
            TuningParam {
                key: "steering_gain".to_string(),
                value: 1.2,
            },
            TuningParam {
                key: "brake_bias".to_string(),
                value: 0.2,
            },
        ];

        let output = policy
            .normalize_tuning(input, StrictMode::Strict)
            .expect("should sort keys");

        assert_eq!(output[0].key, "brake_bias");
        assert_eq!(output[1].key, "steering_gain");
    }
}

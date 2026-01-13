use std::fmt;

#[derive(Debug)]
pub enum PolicyError {
    Io(std::io::Error),
    InvalidYaml(serde_yaml::Error),
    UnsupportedVersion(String),
    MissingParameters,
    UnknownKey(String),
    InvalidValue { key: String, reason: String },
    InvalidField { path: String, reason: String },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyError::Io(err) => write!(f, "failed to read policy file: {err}"),
            PolicyError::InvalidYaml(err) => write!(f, "invalid policy YAML: {err}"),
            PolicyError::UnsupportedVersion(version) => {
                write!(f, "unsupported policy version: {version}")
            }
            PolicyError::MissingParameters => write!(f, "policy parameters must not be empty"),
            PolicyError::UnknownKey(key) => write!(f, "unknown tuning key: {key}"),
            PolicyError::InvalidValue { key, reason } => {
                write!(f, "invalid value for {key}: {reason}")
            }
            PolicyError::InvalidField { path, reason } => {
                write!(f, "invalid field {path}: {reason}")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

pub(crate) fn invalid_field(path: impl Into<String>, reason: impl Into<String>) -> PolicyError {
    PolicyError::InvalidField {
        path: path.into(),
        reason: reason.into(),
    }
}

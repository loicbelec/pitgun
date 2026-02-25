use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimulatorError {
    #[error("missing {kind} config: {id}")]
    MissingConfig { kind: &'static str, id: String },
    #[error("invalid {kind} config '{id}': {reason}")]
    InvalidConfig {
        kind: &'static str,
        id: String,
        reason: String,
    },
    #[error("invalid simulation input: {0}")]
    InvalidInput(String),
    #[error("configuration parse error: {0}")]
    Parse(String),
    #[error("I/O error: {0}")]
    Io(String),
}

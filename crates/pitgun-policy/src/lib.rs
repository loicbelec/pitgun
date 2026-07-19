mod access;
mod tuning;
pub mod validation;

// Access control exports
pub use access::{
    AccessController, AccessDenied, AccessLevel, AccessResult, AuditLogEntry, AuditLogger, Claims,
    InMemoryAuditLog, ParameterAccess, RateLimiter,
};

// TODO: Consider trimming crate-root re-exports once internal users (e.g. configd) migrate.
pub use tuning::{
    CanonicalTuningParameters, ParameterSpec, ParameterSpecV1, PlayerTuningRequest, StrictMode,
    TuningEvalContext, TuningParam, TuningPolicy, TuningPolicyV1,
};
pub use tuning::{
    DerivedConstraint, DeterminismMeta, FloatRange, PolicyError, SigningMeta, TelemetrySchemaHint,
    TuningMeta,
};
pub use tuning::{
    default_policy_path, load_tuning_v1_from_path, load_tuning_v1_from_str, strict_mode_from_env,
};

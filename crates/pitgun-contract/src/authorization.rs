//! Signed authorization metadata for deterministic runs.
//!
//! Authorization is deliberately separate from [`DeterministicRunContractV1`]:
//! issuance time, expiry, subject, audience, nonce and signing-key rotation do
//! not describe the logical computation and therefore must not alter `run_id`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactIdentity, CanonicalJsonError, DeterministicRunContractV1, Digest, Identifier,
    canonical_json_bytes,
};

const MAX_SAFE_JSON_INTEGER: i64 = 9_007_199_254_740_991;

/// Wire version of an authority-issued deterministic-run authorization.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RunAuthorizationVersion {
    /// Initial Pitgun authorization semantics.
    #[serde(rename = "pitgun.run-authorization/v1")]
    V1,
}

/// Signature algorithm declared by a signed authorization.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum AuthorizationSignatureAlgorithm {
    /// HMAC using SHA-256.
    #[serde(rename = "hmac-sha256")]
    HmacSha256,
}

/// Versioned validity window kept outside deterministic run identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationValidityV1 {
    /// Authority wall-clock time at issuance, in Unix epoch milliseconds.
    pub issued_at_ms: i64,
    /// Last instant at which a new execution may start.
    pub expires_at_ms: i64,
    /// Additional time during which a result from an already-authorized
    /// execution may be submitted.
    pub late_submission_grace_ms: i64,
}

impl AuthorizationValidityV1 {
    /// Validates the immutable window independently of the current time.
    pub fn validate(&self) -> Result<(), RunAuthorizationError> {
        if self.issued_at_ms < 0 || self.issued_at_ms > MAX_SAFE_JSON_INTEGER {
            return Err(RunAuthorizationError::InvalidValidity(
                "issued_at_ms must be a non-negative I-JSON safe integer",
            ));
        }
        if self.expires_at_ms <= self.issued_at_ms || self.expires_at_ms > MAX_SAFE_JSON_INTEGER {
            return Err(RunAuthorizationError::InvalidValidity(
                "expires_at_ms must be greater than issued_at_ms and I-JSON safe",
            ));
        }
        if self.late_submission_grace_ms < 0
            || self.late_submission_grace_ms > MAX_SAFE_JSON_INTEGER
            || self
                .expires_at_ms
                .checked_add(self.late_submission_grace_ms)
                .is_none_or(|value| value > MAX_SAFE_JSON_INTEGER)
        {
            return Err(RunAuthorizationError::InvalidValidity(
                "late_submission_grace_ms must produce an I-JSON safe deadline",
            ));
        }
        Ok(())
    }

    /// Validates that a new execution may start at `now_ms`.
    pub fn validate_execution_time(&self, now_ms: i64) -> Result<(), RunAuthorizationError> {
        self.validate()?;
        if now_ms < self.issued_at_ms {
            return Err(RunAuthorizationError::NotYetValid);
        }
        if now_ms > self.expires_at_ms {
            return Err(RunAuthorizationError::Expired);
        }
        Ok(())
    }

    /// Validates that a result may be submitted at `now_ms`.
    pub fn validate_submission_time(&self, now_ms: i64) -> Result<(), RunAuthorizationError> {
        self.validate()?;
        if now_ms < self.issued_at_ms {
            return Err(RunAuthorizationError::NotYetValid);
        }
        let deadline = self.expires_at_ms + self.late_submission_grace_ms;
        if now_ms > deadline {
            return Err(RunAuthorizationError::SubmissionGraceExpired);
        }
        Ok(())
    }
}

/// Canonical bytes authorized by a Pitgun authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAuthorizationV1 {
    /// Exact authorization wire semantics.
    pub authorization_version: RunAuthorizationVersion,
    /// Unpredictable, single-use value used by the verifier to reject replay.
    pub nonce: Digest,
    /// Stable principal for which the run was authorized.
    pub subject: Identifier,
    /// Stable verifier or service expected to accept the result.
    pub audience: Identifier,
    /// Exact deterministic computation authorized by the service.
    pub contract: DeterministicRunContractV1,
    /// Redundant identity that verifiers must recompute from `contract`.
    pub run_id: Digest,
    /// Exact policy artifact used when canonicalizing and authorizing input.
    pub policy: ArtifactIdentity,
    /// Identifier used to select retained verification material after rotation.
    pub signing_key_id: Identifier,
    /// Versioned execution and late-submission window.
    pub validity: AuthorizationValidityV1,
}

impl RunAuthorizationV1 {
    /// Validates all invariants that do not require private verification
    /// material or a replay store.
    pub fn validate_integrity(&self) -> Result<(), RunAuthorizationError> {
        self.validity.validate()?;
        let expected = self.contract.run_id()?;
        if self.run_id != expected {
            return Err(RunAuthorizationError::RunIdMismatch {
                expected,
                actual: self.run_id,
            });
        }
        Ok(())
    }

    /// Returns RFC 8785 canonical bytes protected by the authority signature.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, RunAuthorizationError> {
        self.validate_integrity()?;
        canonical_json_bytes(self).map_err(RunAuthorizationError::CanonicalJson)
    }
}

/// Authority response carrying an authorization and its detached signature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRunAuthorizationV1 {
    /// Canonical authorization payload.
    pub authorization: RunAuthorizationV1,
    /// Declared signature algorithm.
    pub algorithm: AuthorizationSignatureAlgorithm,
    /// Lowercase algorithm-specific signature encoding.
    pub signature: String,
}

/// Validation failures for deterministic-run authorization.
#[derive(Debug)]
pub enum RunAuthorizationError {
    /// The immutable validity window is malformed.
    InvalidValidity(&'static str),
    /// The verifier clock precedes issuance.
    NotYetValid,
    /// The execution authorization has expired.
    Expired,
    /// The result arrived after the documented late-submission window.
    SubmissionGraceExpired,
    /// The redundant identity does not match the canonical contract.
    RunIdMismatch {
        /// Identity recomputed from the canonical contract.
        expected: Digest,
        /// Identity supplied in the signed payload.
        actual: Digest,
    },
    /// Canonical serialization failed.
    CanonicalJson(CanonicalJsonError),
}

impl fmt::Display for RunAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValidity(reason) => {
                write!(formatter, "invalid authorization validity: {reason}")
            }
            Self::NotYetValid => formatter.write_str("authorization is not yet valid"),
            Self::Expired => formatter.write_str("authorization has expired for execution"),
            Self::SubmissionGraceExpired => {
                formatter.write_str("authorization late-submission grace has expired")
            }
            Self::RunIdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "authorization run_id mismatch: expected {expected}, got {actual}"
                )
            }
            Self::CanonicalJson(error) => {
                write!(formatter, "cannot canonicalize authorization: {error}")
            }
        }
    }
}

impl std::error::Error for RunAuthorizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CanonicalJson(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CanonicalJsonError> for RunAuthorizationError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactIdentity, ContractVersion, EventOrderingV1, InputCanonicalization, InputIdentity,
        InputMediaType, LogicalClockV1, RandomAlgorithm, RandomContractV1, RuntimeProfile,
        ScenarioIdentity, Seed, StreamDerivation,
    };

    fn artifact(id: &str, bytes: &[u8]) -> ArtifactIdentity {
        ArtifactIdentity {
            id: id.parse().expect("artifact id"),
            version: "1.0.0".parse().expect("artifact version"),
            digest: Digest::from_bytes(bytes),
        }
    }

    fn authorization() -> RunAuthorizationV1 {
        let contract = DeterministicRunContractV1 {
            contract_version: ContractVersion::V1,
            scenario: ScenarioIdentity {
                id: "racing.weekend".parse().expect("scenario id"),
                version: "1.0.0".parse().expect("scenario version"),
            },
            model: artifact("pitgun.racing", b"model"),
            data_pack: artifact("pitgun.racing.simulation", b"data-pack"),
            runtime_profile: RuntimeProfile::PortableExactV1,
            random: RandomContractV1 {
                seed: Seed::new(42),
                algorithm: RandomAlgorithm::PitgunSplitMix64V1,
                stream_derivation: StreamDerivation::Sha256LabelV1,
            },
            clock: LogicalClockV1::new(0, 50_000, 1).expect("clock"),
            event_ordering: EventOrderingV1::v1(),
            input: InputIdentity {
                media_type: InputMediaType::ApplicationJson,
                canonicalization: InputCanonicalization::JcsRfc8785,
                digest: Digest::from_bytes(b"input"),
            },
        };
        let run_id = contract.run_id().expect("run id");
        RunAuthorizationV1 {
            authorization_version: RunAuthorizationVersion::V1,
            nonce: Digest::from_bytes(b"nonce"),
            subject: "career.123".parse().expect("subject"),
            audience: "pitgun.verifier".parse().expect("audience"),
            contract,
            run_id,
            policy: artifact("pitgun.racing.policy", b"policy"),
            signing_key_id: "staging-2026-07".parse().expect("key id"),
            validity: AuthorizationValidityV1 {
                issued_at_ms: 1_710_000_000_000,
                expires_at_ms: 1_710_000_300_000,
                late_submission_grace_ms: 120_000,
            },
        }
    }

    #[test]
    fn authorization_has_stable_canonical_signing_bytes() {
        let first = authorization();
        let json = serde_json::to_string_pretty(&first).expect("pretty authorization");
        let decoded: RunAuthorizationV1 =
            serde_json::from_str(&json).expect("strict authorization");

        assert_eq!(
            first.signing_bytes().expect("first signing bytes"),
            decoded.signing_bytes().expect("decoded signing bytes")
        );
    }

    #[test]
    fn forged_run_id_fails_closed() {
        let mut value = authorization();
        value.run_id = Digest::from_bytes(b"forged");

        assert!(matches!(
            value.signing_bytes(),
            Err(RunAuthorizationError::RunIdMismatch { .. })
        ));
    }

    #[test]
    fn execution_and_submission_have_distinct_deadlines() {
        let value = authorization();
        assert!(
            value
                .validity
                .validate_execution_time(1_710_000_300_000)
                .is_ok()
        );
        assert!(matches!(
            value.validity.validate_execution_time(1_710_000_300_001),
            Err(RunAuthorizationError::Expired)
        ));
        assert!(
            value
                .validity
                .validate_submission_time(1_710_000_420_000)
                .is_ok()
        );
        assert!(matches!(
            value.validity.validate_submission_time(1_710_000_420_001),
            Err(RunAuthorizationError::SubmissionGraceExpired)
        ));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let mut value = serde_json::to_value(authorization()).expect("authorization value");
        value
            .as_object_mut()
            .expect("authorization object")
            .insert("catalog_url".to_string(), serde_json::json!("latest"));

        assert!(serde_json::from_value::<RunAuthorizationV1>(value).is_err());
    }
}

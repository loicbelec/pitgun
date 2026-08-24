//! Signed authorization for a dynamic attempt whose final input is not known at issuance.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactIdentity, AuthorizationSignatureAlgorithm, AuthorizationValidityV1, CanonicalJsonError,
    DeterministicRunContractV1, Digest, ExecutionId, ExecutionReceiptV1, Identifier,
    RunAuthorizationError, RunContractError, canonical_json_bytes,
};

/// Wire version of an authority-issued dynamic attempt authorization.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RunAttemptAuthorizationVersion {
    #[serde(rename = "pitgun.run-attempt-authorization/v1")]
    V1,
}

/// Canonical bytes authorized before a dynamic run's final input exists.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAttemptAuthorizationV1 {
    pub authorization_version: RunAttemptAuthorizationVersion,
    /// Unpredictable single-use value consumed when accepting one submission.
    pub nonce: Digest,
    /// Concrete attempt fixed before domain decisions are applied.
    pub execution_id: ExecutionId,
    pub subject: Identifier,
    pub audience: Identifier,
    /// Immutable computation semantics and input known at issuance.
    pub initial_contract: DeterministicRunContractV1,
    /// Redundant identity that must be recomputed before signing or verification.
    pub initial_run_id: Digest,
    /// Content-addressed domain constraints interpreted by the selected workload.
    pub decision_envelope: ArtifactIdentity,
    pub policy: ArtifactIdentity,
    pub signing_key_id: Identifier,
    pub validity: AuthorizationValidityV1,
}

/// Authority response carrying a dynamic attempt authorization and signature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRunAttemptAuthorizationV1 {
    pub authorization: RunAttemptAuthorizationV1,
    pub algorithm: AuthorizationSignatureAlgorithm,
    pub signature: String,
}

#[derive(Debug)]
pub enum RunAttemptAuthorizationError {
    Authorization(RunAuthorizationError),
    InitialRunIdMismatch {
        expected: Digest,
        actual: Digest,
    },
    FinalContractSemanticMismatch,
    ExecutionIdMismatch {
        expected: ExecutionId,
        actual: ExecutionId,
    },
    Receipt(RunContractError),
    CanonicalJson(CanonicalJsonError),
}

impl fmt::Display for RunAttemptAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authorization(error) => error.fmt(formatter),
            Self::InitialRunIdMismatch { expected, actual } => write!(
                formatter,
                "attempt authorization initial_run_id mismatch: expected {expected}, got {actual}",
            ),
            Self::FinalContractSemanticMismatch => formatter.write_str(
                "completed contract changes semantics outside the authorized input digest",
            ),
            Self::ExecutionIdMismatch { expected, actual } => write!(
                formatter,
                "completed receipt execution_id mismatch: expected {expected}, got {actual}",
            ),
            Self::Receipt(error) => error.fmt(formatter),
            Self::CanonicalJson(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RunAttemptAuthorizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authorization(error) => Some(error),
            Self::Receipt(error) => Some(error),
            Self::CanonicalJson(error) => Some(error),
            Self::InitialRunIdMismatch { .. }
            | Self::FinalContractSemanticMismatch
            | Self::ExecutionIdMismatch { .. } => None,
        }
    }
}

impl From<CanonicalJsonError> for RunAttemptAuthorizationError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

impl RunAttemptAuthorizationV1 {
    /// Validates invariants independent of private key material or current time.
    pub fn validate_integrity(&self) -> Result<(), RunAttemptAuthorizationError> {
        self.validity
            .validate()
            .map_err(RunAttemptAuthorizationError::Authorization)?;
        let expected = self.initial_contract.run_id()?;
        if self.initial_run_id != expected {
            return Err(RunAttemptAuthorizationError::InitialRunIdMismatch {
                expected,
                actual: self.initial_run_id,
            });
        }
        Ok(())
    }

    pub fn validate_execution_time(&self, now_ms: i64) -> Result<(), RunAttemptAuthorizationError> {
        self.validate_integrity()?;
        self.validity
            .validate_execution_time(now_ms)
            .map_err(RunAttemptAuthorizationError::Authorization)
    }

    pub fn validate_submission_time(
        &self,
        now_ms: i64,
    ) -> Result<(), RunAttemptAuthorizationError> {
        self.validate_integrity()?;
        self.validity
            .validate_submission_time(now_ms)
            .map_err(RunAttemptAuthorizationError::Authorization)
    }

    /// Returns RFC 8785 canonical bytes protected by the authority signature.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, RunAttemptAuthorizationError> {
        self.validate_integrity()?;
        Ok(canonical_json_bytes(self)?)
    }

    /// Verifies the generic boundary between initial and completed contracts.
    ///
    /// A dynamic workload may replace only the canonical input digest. Model,
    /// data pack, RNG, clock, ordering, media type and canonicalization remain
    /// exactly those authorized before execution.
    pub fn validate_final_contract(
        &self,
        final_contract: &DeterministicRunContractV1,
    ) -> Result<(), RunAttemptAuthorizationError> {
        self.validate_integrity()?;
        if final_contract.input.media_type != self.initial_contract.input.media_type
            || final_contract.input.canonicalization != self.initial_contract.input.canonicalization
        {
            return Err(RunAttemptAuthorizationError::FinalContractSemanticMismatch);
        }

        let mut normalized = final_contract.clone();
        normalized.input.digest = self.initial_contract.input.digest;
        if normalized != self.initial_contract {
            return Err(RunAttemptAuthorizationError::FinalContractSemanticMismatch);
        }
        Ok(())
    }

    /// Binds a completed receipt to both the final contract and authorized attempt.
    pub fn validate_completed_receipt(
        &self,
        final_contract: &DeterministicRunContractV1,
        receipt: &ExecutionReceiptV1,
    ) -> Result<(), RunAttemptAuthorizationError> {
        self.validate_final_contract(final_contract)?;
        if receipt.execution_id != self.execution_id {
            return Err(RunAttemptAuthorizationError::ExecutionIdMismatch {
                expected: self.execution_id,
                actual: receipt.execution_id,
            });
        }
        final_contract
            .verify_receipt(receipt)
            .map_err(RunAttemptAuthorizationError::Receipt)
    }
}

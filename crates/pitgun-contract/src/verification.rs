//! Versioned decisions produced by a trusted Pitgun verifier.
//!
//! A verdict records what was submitted and what the verifier decided. It is
//! not accepted as client input and it does not replace the underlying
//! authorization, receipt, or deterministic evidence.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ArtifactIdentity, Digest, ExecutionId};

const MAX_SAFE_JSON_INTEGER: i64 = 9_007_199_254_740_991;

/// Wire version of a hosted verification verdict.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum VerificationVerdictVersion {
    /// Initial hosted-verification semantics.
    #[serde(rename = "pitgun.verification-verdict/v1")]
    V1,
}

/// Server-owned lifecycle state of one submitted execution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Verification has not reached a terminal decision and may be retried.
    #[serde(rename = "PENDING")]
    Pending,
    /// All required evidence and deterministic replay checks passed.
    #[serde(rename = "VERIFIED")]
    Verified,
    /// A terminal, fail-closed verification check failed.
    #[serde(rename = "REJECTED")]
    Rejected,
}

/// Stable machine-readable explanation for a non-verified state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationReasonCode {
    /// Work was accepted and is waiting for verifier capacity.
    VerificationQueued,
    /// The immutable catalog release could not be retrieved temporarily.
    CatalogUnavailable,
    /// The exact model or simulation pack could not be retrieved temporarily.
    ModelUnavailable,
    /// An internal dependency required for verification is temporarily down.
    DependencyUnavailable,
    /// The authority signature, audience, or authorization structure is invalid.
    InvalidAuthorization,
    /// The submission arrived outside the authorization validity window.
    AuthorizationExpired,
    /// The authorization nonce was already consumed by another execution.
    AuthorizationReplayed,
    /// The contract does not recompute to the declared logical run identity.
    RunIdMismatch,
    /// The model identity is not retained or accepted by this verifier.
    UnknownModel,
    /// The simulation-pack identity is not retained or accepted by this verifier.
    UnknownDataPack,
    /// The policy identity is not retained or accepted by this verifier.
    UnknownPolicy,
    /// Submitted evidence cannot be decoded or violates its wire contract.
    EvidenceMalformed,
    /// Submitted bytes do not match their declared content digest.
    ArtifactDigestMismatch,
    /// The execution receipt is not bound to the submitted contract or evidence.
    ReceiptMismatch,
    /// The canonical domain output differs from independent replay.
    OutputMismatch,
    /// The canonical telemetry summary differs from independent replay.
    TelemetryMismatch,
    /// Independent deterministic replay could not reproduce the execution.
    ReplayMismatch,
}

impl VerificationReasonCode {
    /// Returns whether this reason represents retryable, non-terminal work.
    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(
            self,
            Self::VerificationQueued
                | Self::CatalogUnavailable
                | Self::ModelUnavailable
                | Self::DependencyUnavailable
        )
    }

    /// Returns whether this reason represents a terminal rejection.
    #[must_use]
    pub const fn is_rejection(self) -> bool {
        !self.is_pending()
    }
}

/// Content identities calculated for the exact submitted evidence bytes.
///
/// These digests document what the verifier received. Their presence does not
/// imply that the evidence was valid.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmittedEvidenceV1 {
    /// Digest of the canonical execution receipt artifact.
    pub receipt_digest: Digest,
    /// Digest of the complete canonical domain output artifact.
    pub output_digest: Digest,
    /// Digest of the canonical telemetry-summary artifact.
    pub telemetry_summary_digest: Digest,
}

/// Exact resources independently resolved and accepted by the verifier.
///
/// This block is present only for a `VERIFIED` verdict.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedResolutionV1 {
    /// Exact executable model used for independent replay.
    pub model: ArtifactIdentity,
    /// Exact immutable data or simulation pack used for replay.
    pub data_pack: ArtifactIdentity,
    /// Exact authority policy bound to the accepted authorization.
    pub policy: ArtifactIdentity,
}

/// Durable verifier-owned decision for one concrete execution attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationVerdictV1 {
    /// Exact verdict wire semantics.
    pub schema_version: VerificationVerdictVersion,
    /// Logical deterministic computation shared by equivalent attempts.
    pub run_id: Digest,
    /// Concrete browser, native, or worker execution being assessed.
    pub execution_id: ExecutionId,
    /// Current server-owned decision.
    pub status: VerificationStatus,
    /// Required for pending and rejected decisions; absent when verified.
    pub reason_code: Option<VerificationReasonCode>,
    /// Content identities of the exact evidence received by the verifier.
    pub submitted_evidence: SubmittedEvidenceV1,
    /// Trusted replay resolution, present only after successful verification.
    pub verified_resolution: Option<VerifiedResolutionV1>,
    /// Exact verifier implementation producing this record.
    pub verifier: ArtifactIdentity,
    /// Unix epoch milliseconds at which this record was produced.
    ///
    /// This operational timestamp is not part of deterministic run identity.
    pub recorded_at_ms: i64,
}

impl VerificationVerdictV1 {
    /// Validates state-dependent and portable JSON invariants.
    pub fn validate(&self) -> Result<(), VerificationVerdictError> {
        if self.recorded_at_ms < 0 || self.recorded_at_ms > MAX_SAFE_JSON_INTEGER {
            return Err(VerificationVerdictError::InvalidRecordedAt);
        }

        match self.status {
            VerificationStatus::Pending => {
                let reason = self
                    .reason_code
                    .ok_or(VerificationVerdictError::MissingReason(
                        VerificationStatus::Pending,
                    ))?;
                if !reason.is_pending() {
                    return Err(VerificationVerdictError::ReasonDoesNotMatchStatus {
                        status: self.status,
                        reason,
                    });
                }
                if self.verified_resolution.is_some() {
                    return Err(VerificationVerdictError::UnexpectedVerifiedResolution(
                        self.status,
                    ));
                }
            }
            VerificationStatus::Verified => {
                if self.reason_code.is_some() {
                    return Err(VerificationVerdictError::UnexpectedReason(self.status));
                }
                if self.verified_resolution.is_none() {
                    return Err(VerificationVerdictError::MissingVerifiedResolution);
                }
            }
            VerificationStatus::Rejected => {
                let reason = self
                    .reason_code
                    .ok_or(VerificationVerdictError::MissingReason(
                        VerificationStatus::Rejected,
                    ))?;
                if !reason.is_rejection() {
                    return Err(VerificationVerdictError::ReasonDoesNotMatchStatus {
                        status: self.status,
                        reason,
                    });
                }
                if self.verified_resolution.is_some() {
                    return Err(VerificationVerdictError::UnexpectedVerifiedResolution(
                        self.status,
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Invalid state in a verifier-owned verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationVerdictError {
    /// The operational timestamp is negative or cannot round-trip through I-JSON.
    InvalidRecordedAt,
    /// A non-verified state lacks its machine-readable reason.
    MissingReason(VerificationStatus),
    /// A verified state incorrectly carries a failure reason.
    UnexpectedReason(VerificationStatus),
    /// A reason was paired with the wrong lifecycle state.
    ReasonDoesNotMatchStatus {
        status: VerificationStatus,
        reason: VerificationReasonCode,
    },
    /// A verified decision lacks the exact trusted resources used for replay.
    MissingVerifiedResolution,
    /// A non-verified decision claims resources were successfully verified.
    UnexpectedVerifiedResolution(VerificationStatus),
}

impl fmt::Display for VerificationVerdictError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRecordedAt => {
                formatter.write_str("recorded_at_ms must be a non-negative I-JSON safe integer")
            }
            Self::MissingReason(status) => {
                write!(formatter, "{status:?} verdict requires a reason_code")
            }
            Self::UnexpectedReason(status) => {
                write!(
                    formatter,
                    "{status:?} verdict must not contain a reason_code"
                )
            }
            Self::ReasonDoesNotMatchStatus { status, reason } => {
                write!(formatter, "reason {reason:?} is not valid for {status:?}")
            }
            Self::MissingVerifiedResolution => {
                formatter.write_str("VERIFIED verdict requires verified_resolution")
            }
            Self::UnexpectedVerifiedResolution(status) => write!(
                formatter,
                "{status:?} verdict must not contain verified_resolution"
            ),
        }
    }
}

impl std::error::Error for VerificationVerdictError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, SemanticVersion, canonical_json_digest};

    fn artifact(id: &str, bytes: &[u8]) -> ArtifactIdentity {
        ArtifactIdentity {
            id: id.parse::<Identifier>().expect("artifact id"),
            version: SemanticVersion::new("1.0.0").expect("artifact version"),
            digest: Digest::from_bytes(bytes),
        }
    }

    fn evidence() -> SubmittedEvidenceV1 {
        SubmittedEvidenceV1 {
            receipt_digest: Digest::from_bytes(b"receipt"),
            output_digest: Digest::from_bytes(b"output"),
            telemetry_summary_digest: Digest::from_bytes(b"telemetry-summary"),
        }
    }

    fn verdict(status: VerificationStatus) -> VerificationVerdictV1 {
        VerificationVerdictV1 {
            schema_version: VerificationVerdictVersion::V1,
            run_id: Digest::from_bytes(b"run"),
            execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
                .parse()
                .expect("execution id"),
            status,
            reason_code: None,
            submitted_evidence: evidence(),
            verified_resolution: None,
            verifier: artifact("pitgun.verifier", b"verifier"),
            recorded_at_ms: 1_722_345_678_901,
        }
    }

    #[test]
    fn verified_verdict_requires_resolution_and_no_reason() {
        let mut value = verdict(VerificationStatus::Verified);
        assert_eq!(
            value.validate(),
            Err(VerificationVerdictError::MissingVerifiedResolution)
        );

        value.verified_resolution = Some(VerifiedResolutionV1 {
            model: artifact("pitgun.racing", b"model"),
            data_pack: artifact("pitgun.racing.simulation", b"data-pack"),
            policy: artifact("pitgun.racing.tuning", b"policy"),
        });
        assert_eq!(value.validate(), Ok(()));

        value.reason_code = Some(VerificationReasonCode::ReplayMismatch);
        assert_eq!(
            value.validate(),
            Err(VerificationVerdictError::UnexpectedReason(
                VerificationStatus::Verified
            ))
        );
    }

    #[test]
    fn pending_verdict_accepts_only_retryable_reasons() {
        let mut value = verdict(VerificationStatus::Pending);
        value.reason_code = Some(VerificationReasonCode::CatalogUnavailable);
        assert_eq!(value.validate(), Ok(()));

        value.reason_code = Some(VerificationReasonCode::OutputMismatch);
        assert_eq!(
            value.validate(),
            Err(VerificationVerdictError::ReasonDoesNotMatchStatus {
                status: VerificationStatus::Pending,
                reason: VerificationReasonCode::OutputMismatch,
            })
        );
    }

    #[test]
    fn rejected_verdict_accepts_only_terminal_reasons() {
        let mut value = verdict(VerificationStatus::Rejected);
        value.reason_code = Some(VerificationReasonCode::TelemetryMismatch);
        assert_eq!(value.validate(), Ok(()));

        value.reason_code = Some(VerificationReasonCode::DependencyUnavailable);
        assert_eq!(
            value.validate(),
            Err(VerificationVerdictError::ReasonDoesNotMatchStatus {
                status: VerificationStatus::Rejected,
                reason: VerificationReasonCode::DependencyUnavailable,
            })
        );
    }

    #[test]
    fn verdict_round_trips_with_stable_wire_values() {
        let mut value = verdict(VerificationStatus::Verified);
        value.verified_resolution = Some(VerifiedResolutionV1 {
            model: artifact("pitgun.racing", b"model"),
            data_pack: artifact("pitgun.racing.simulation", b"data-pack"),
            policy: artifact("pitgun.racing.tuning", b"policy"),
        });

        let json = serde_json::to_value(&value).expect("serialize verdict");
        assert_eq!(json["schema_version"], "pitgun.verification-verdict/v1");
        assert_eq!(json["status"], "VERIFIED");
        assert!(json["reason_code"].is_null());

        let decoded: VerificationVerdictV1 =
            serde_json::from_value(json).expect("deserialize verdict");
        assert_eq!(decoded, value);
        assert_eq!(decoded.validate(), Ok(()));

        let fixture: VerificationVerdictV1 = serde_json::from_str(include_str!(
            "../tests/fixtures/verification-verdict-v1/verified.json"
        ))
        .expect("published verified verdict fixture");
        assert_eq!(fixture, value);
        assert_eq!(fixture.validate(), Ok(()));
        assert_eq!(
            canonical_json_digest(&fixture)
                .expect("canonical verdict digest")
                .to_string(),
            "sha256:46b366019b8a32eb9381c7a99767e263898483177e045249a4be1f6a88d3e3fd"
        );
    }

    #[test]
    fn verdict_rejects_unknown_wire_fields() {
        let mut value = verdict(VerificationStatus::Pending);
        value.reason_code = Some(VerificationReasonCode::VerificationQueued);
        let mut json = serde_json::to_value(value).expect("serialize verdict");
        json["client_verified"] = serde_json::json!(true);

        assert!(serde_json::from_value::<VerificationVerdictV1>(json).is_err());
    }

    #[test]
    fn verdict_rejects_non_portable_timestamp() {
        let mut value = verdict(VerificationStatus::Pending);
        value.reason_code = Some(VerificationReasonCode::VerificationQueued);
        value.recorded_at_ms = MAX_SAFE_JSON_INTEGER + 1;

        assert_eq!(
            value.validate(),
            Err(VerificationVerdictError::InvalidRecordedAt)
        );
    }
}

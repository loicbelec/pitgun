//! Hosted Racing verification engine.
//!
//! Transport, persistence, nonce consumption, and leaderboard projection sit
//! outside this module. The engine accepts typed evidence and emits the
//! server-owned, versioned verification verdict.

use std::fmt;

use pitgun_contract::{
    ArtifactIdentity, CanonicalJsonError, ContractVersion, EventOrderingV1, InputCanonicalization,
    InputMediaType, LogicalClockV1, RandomAlgorithm, RunAttemptAuthorizationError,
    RunAuthorizationError, RuntimeProfile, StreamDerivation, SubmittedEvidenceV1,
    VerificationReasonCode, VerificationStatus, VerificationVerdictError, VerificationVerdictV1,
    VerificationVerdictVersion, VerifiedResolutionV1, canonical_json_digest,
};
use pitgun_racing_simulator::evidence::{RacingExecutionResolutionV1, RacingRunEvidenceV1};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RacingWorkload, RunRaceInput, RunRaceRequest,
    run_race_with_catalog_and_v3_timeline_candidate,
};
use pitgun_runtime::{LinkedWorkloadError, execute_linked};
use pitgun_signing::{AuthorizationVerificationError, VerificationKeyring};

pub use pitgun_racing_simulator::evidence::{
    RacingDynamicVerificationSubmissionV1, RacingVerificationSubmissionV1,
};

const RACING_SCENARIO_ID: &str = "racing.race";
const RACING_DYNAMIC_SCENARIO_ID: &str = "racing.dynamic-session";
const RACING_SCENARIO_VERSION: &str = "1.0.0";

/// Trusted dependencies and retained identities used to verify Racing V1.
#[derive(Clone, Debug)]
pub struct RacingVerifier {
    authorization_keys: VerificationKeyring,
    expected_audience: pitgun_contract::Identifier,
    expected_model: ArtifactIdentity,
    policy: ArtifactIdentity,
    catalog: Option<RacingCatalogSnapshot>,
    verifier: ArtifactIdentity,
}

impl RacingVerifier {
    /// Creates a verifier with explicit retained trust material.
    #[must_use]
    pub const fn new(
        authorization_keys: VerificationKeyring,
        expected_audience: pitgun_contract::Identifier,
        expected_model: ArtifactIdentity,
        policy: ArtifactIdentity,
        catalog: Option<RacingCatalogSnapshot>,
        verifier: ArtifactIdentity,
    ) -> Self {
        Self {
            authorization_keys,
            expected_audience,
            expected_model,
            policy,
            catalog,
            verifier,
        }
    }

    /// Independently validates and replays one submitted Racing execution.
    pub fn verify(
        &self,
        submission: &RacingVerificationSubmissionV1,
        now_ms: i64,
    ) -> Result<VerificationVerdictV1, RacingVerifierError> {
        let submitted_evidence = submitted_evidence(submission)?;

        if let Err(error) = self.authorization_keys.verify_submission(
            &submission.signed_authorization,
            &self.expected_audience,
            now_ms,
        ) {
            return self.verdict(
                submission,
                submitted_evidence,
                VerificationStatus::Rejected,
                Some(authorization_reason(&error)),
                None,
                now_ms,
            );
        }

        let authorization = &submission.signed_authorization.authorization;
        let contract = &authorization.contract;

        if !has_supported_contract_shape(contract, &submission.input) {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::InvalidAuthorization,
                now_ms,
            );
        }
        if contract.model != self.expected_model {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::UnknownModel,
                now_ms,
            );
        }
        if authorization.policy != self.policy {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::UnknownPolicy,
                now_ms,
            );
        }

        let Some(catalog) = &self.catalog else {
            return self.verdict(
                submission,
                submitted_evidence,
                VerificationStatus::Pending,
                Some(VerificationReasonCode::CatalogUnavailable),
                None,
                now_ms,
            );
        };
        if contract.data_pack != catalog.manifest().simulation_pack.identity {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::UnknownDataPack,
                now_ms,
            );
        }
        if catalog.manifest().validate_for_run(contract).is_err() {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::UnknownDataPack,
                now_ms,
            );
        }
        let expected_execution_resolution =
            RacingExecutionResolutionV1::from_catalog(catalog, &contract.model);
        if submission.execution_resolution != expected_execution_resolution {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::ArtifactDigestMismatch,
                now_ms,
            );
        }

        let input_digest = canonical_json_digest(&submission.input)?;
        if input_digest != contract.input.digest {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::ArtifactDigestMismatch,
                now_ms,
            );
        }
        if contract
            .verify_receipt(&submission.receipt.receipt)
            .is_err()
            || submission.receipt.receipt.run_id != authorization.run_id
        {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::ReceiptMismatch,
                now_ms,
            );
        }
        if submission.receipt.receipt.output_digest != submitted_evidence.output_digest {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::OutputMismatch,
                now_ms,
            );
        }
        if submission.receipt.receipt.telemetry_summary_digest
            != submitted_evidence.telemetry_summary_digest
        {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::TelemetryMismatch,
                now_ms,
            );
        }

        let workload = match RacingWorkload::for_model(&self.expected_model, catalog.clone()) {
            Ok(workload) => workload,
            Err(_) => {
                return self.rejected(
                    submission,
                    submitted_evidence,
                    VerificationReasonCode::UnknownModel,
                    now_ms,
                );
            }
        };
        let replay = match execute_linked(&workload, contract, submission.input.clone()) {
            Ok(replay) => replay,
            Err(error) => {
                return self.rejected(
                    submission,
                    submitted_evidence,
                    replay_reason(&error),
                    now_ms,
                );
            }
        };
        if replay.output_digest != submitted_evidence.output_digest
            || replay.evidence.output != submission.output
        {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::OutputMismatch,
                now_ms,
            );
        }
        if replay.telemetry_summary_digest != submitted_evidence.telemetry_summary_digest
            || replay.evidence.telemetry_summary != submission.telemetry_summary
        {
            return self.rejected(
                submission,
                submitted_evidence,
                VerificationReasonCode::TelemetryMismatch,
                now_ms,
            );
        }

        self.verdict(
            submission,
            submitted_evidence,
            VerificationStatus::Verified,
            None,
            Some(VerifiedResolutionV1 {
                model: contract.model.clone(),
                data_pack: contract.data_pack.clone(),
                policy: authorization.policy.clone(),
            }),
            now_ms,
        )
    }

    /// Independently validates and replays one catalog-governed dynamic attempt.
    pub fn verify_attempt(
        &self,
        submission: &RacingDynamicVerificationSubmissionV1,
        now_ms: i64,
    ) -> Result<VerificationVerdictV1, RacingVerifierError> {
        let submitted_evidence = submitted_dynamic_evidence(submission)?;
        let attempt = &submission.signed_authorization.authorization;
        let initial_contract = &attempt.initial_contract;

        if let Err(error) = self.authorization_keys.verify_attempt_submission(
            &submission.signed_authorization,
            &self.expected_audience,
            now_ms,
        ) {
            return self.attempt_verdict(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationStatus::Rejected,
                Some(authorization_reason(&error)),
                None,
                now_ms,
            );
        }
        if !has_supported_dynamic_contract_shape(initial_contract, &submission.input) {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::InvalidAuthorization,
                now_ms,
            );
        }
        if initial_contract.model != self.expected_model {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::UnknownModel,
                now_ms,
            );
        }
        if attempt.policy != self.policy {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::UnknownPolicy,
                now_ms,
            );
        }

        let Some(catalog) = &self.catalog else {
            return self.attempt_verdict(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationStatus::Pending,
                Some(VerificationReasonCode::CatalogUnavailable),
                None,
                now_ms,
            );
        };
        if initial_contract.data_pack != catalog.manifest().simulation_pack.identity
            || catalog
                .manifest()
                .validate_for_run(initial_contract)
                .is_err()
        {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::UnknownDataPack,
                now_ms,
            );
        }
        let expected_resolution =
            RacingExecutionResolutionV1::from_catalog(catalog, &initial_contract.model);
        if Some(&submission.execution_resolution) != expected_resolution.as_ref() {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::ArtifactDigestMismatch,
                now_ms,
            );
        }
        if canonical_json_digest(&submission.input)? != initial_contract.input.digest {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::ArtifactDigestMismatch,
                now_ms,
            );
        }

        let Some(instruction_profile_identity) = catalog.driver_instruction_profile_identity()
        else {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::UnknownDataPack,
                now_ms,
            );
        };
        let Some(instruction_profile) = catalog.driver_instruction_profile() else {
            return self.rejected_attempt(
                attempt.initial_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::UnknownDataPack,
                now_ms,
            );
        };
        let final_contract = match submission.decision_envelope.final_contract_for_attempt(
            attempt,
            &submission.completed_input,
            instruction_profile_identity,
            instruction_profile,
        ) {
            Ok(contract) => contract,
            Err(_) => {
                return self.rejected_attempt(
                    attempt.initial_run_id,
                    attempt.execution_id,
                    submitted_evidence,
                    VerificationReasonCode::InvalidAuthorization,
                    now_ms,
                );
            }
        };
        let final_run_id = canonical_json_digest(&final_contract)?;
        if attempt
            .validate_completed_receipt(&final_contract, &submission.receipt.receipt)
            .is_err()
        {
            return self.rejected_attempt(
                final_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::ReceiptMismatch,
                now_ms,
            );
        }
        if submission.receipt.receipt.output_digest != submitted_evidence.output_digest {
            return self.rejected_attempt(
                final_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::OutputMismatch,
                now_ms,
            );
        }
        if submission.receipt.receipt.telemetry_summary_digest
            != submitted_evidence.telemetry_summary_digest
        {
            return self.rejected_attempt(
                final_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::TelemetryMismatch,
                now_ms,
            );
        }

        let replay = match run_race_with_catalog_and_v3_timeline_candidate(
            RunRaceRequest {
                input: submission.input.clone(),
                seed: initial_contract.random.seed.get(),
                era: Some(submission.input.era),
                hz: Some(submission.input.hz),
            },
            catalog,
            submission
                .completed_input
                .driver_instructions
                .applied_timeline
                .clone(),
        ) {
            Ok(output) => output,
            Err(_) => {
                return self.rejected_attempt(
                    final_run_id,
                    attempt.execution_id,
                    submitted_evidence,
                    VerificationReasonCode::ReplayMismatch,
                    now_ms,
                );
            }
        };
        let replay_evidence = match RacingRunEvidenceV1::from_race_output(&replay) {
            Ok(evidence) => evidence,
            Err(_) => {
                return self.rejected_attempt(
                    final_run_id,
                    attempt.execution_id,
                    submitted_evidence,
                    VerificationReasonCode::ReplayMismatch,
                    now_ms,
                );
            }
        };
        if replay_evidence.output != submission.output
            || replay_evidence.output_digest()? != submitted_evidence.output_digest
        {
            return self.rejected_attempt(
                final_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::OutputMismatch,
                now_ms,
            );
        }
        if replay_evidence.telemetry_summary != submission.telemetry_summary
            || replay_evidence.telemetry_summary_digest()?
                != submitted_evidence.telemetry_summary_digest
        {
            return self.rejected_attempt(
                final_run_id,
                attempt.execution_id,
                submitted_evidence,
                VerificationReasonCode::TelemetryMismatch,
                now_ms,
            );
        }

        self.attempt_verdict(
            final_run_id,
            attempt.execution_id,
            submitted_evidence,
            VerificationStatus::Verified,
            None,
            Some(VerifiedResolutionV1 {
                model: initial_contract.model.clone(),
                data_pack: initial_contract.data_pack.clone(),
                policy: attempt.policy.clone(),
            }),
            now_ms,
        )
    }

    fn rejected_attempt(
        &self,
        run_id: pitgun_contract::Digest,
        execution_id: pitgun_contract::ExecutionId,
        submitted_evidence: SubmittedEvidenceV1,
        reason: VerificationReasonCode,
        now_ms: i64,
    ) -> Result<VerificationVerdictV1, RacingVerifierError> {
        self.attempt_verdict(
            run_id,
            execution_id,
            submitted_evidence,
            VerificationStatus::Rejected,
            Some(reason),
            None,
            now_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn attempt_verdict(
        &self,
        run_id: pitgun_contract::Digest,
        execution_id: pitgun_contract::ExecutionId,
        submitted_evidence: SubmittedEvidenceV1,
        status: VerificationStatus,
        reason_code: Option<VerificationReasonCode>,
        verified_resolution: Option<VerifiedResolutionV1>,
        now_ms: i64,
    ) -> Result<VerificationVerdictV1, RacingVerifierError> {
        let verdict = VerificationVerdictV1 {
            schema_version: VerificationVerdictVersion::V1,
            run_id,
            execution_id,
            status,
            reason_code,
            submitted_evidence,
            verified_resolution,
            verifier: self.verifier.clone(),
            recorded_at_ms: now_ms,
        };
        verdict.validate()?;
        Ok(verdict)
    }

    fn rejected(
        &self,
        submission: &RacingVerificationSubmissionV1,
        submitted_evidence: SubmittedEvidenceV1,
        reason: VerificationReasonCode,
        now_ms: i64,
    ) -> Result<VerificationVerdictV1, RacingVerifierError> {
        self.verdict(
            submission,
            submitted_evidence,
            VerificationStatus::Rejected,
            Some(reason),
            None,
            now_ms,
        )
    }

    fn verdict(
        &self,
        submission: &RacingVerificationSubmissionV1,
        submitted_evidence: SubmittedEvidenceV1,
        status: VerificationStatus,
        reason_code: Option<VerificationReasonCode>,
        verified_resolution: Option<VerifiedResolutionV1>,
        now_ms: i64,
    ) -> Result<VerificationVerdictV1, RacingVerifierError> {
        let verdict = VerificationVerdictV1 {
            schema_version: VerificationVerdictVersion::V1,
            run_id: submission.signed_authorization.authorization.run_id,
            execution_id: submission.receipt.receipt.execution_id,
            status,
            reason_code,
            submitted_evidence,
            verified_resolution,
            verifier: self.verifier.clone(),
            recorded_at_ms: now_ms,
        };
        verdict.validate()?;
        Ok(verdict)
    }
}

fn submitted_evidence(
    submission: &RacingVerificationSubmissionV1,
) -> Result<SubmittedEvidenceV1, CanonicalJsonError> {
    Ok(SubmittedEvidenceV1 {
        receipt_digest: canonical_json_digest(&submission.receipt)?,
        output_digest: canonical_json_digest(&submission.output)?,
        telemetry_summary_digest: canonical_json_digest(&submission.telemetry_summary)?,
    })
}

fn submitted_dynamic_evidence(
    submission: &RacingDynamicVerificationSubmissionV1,
) -> Result<SubmittedEvidenceV1, CanonicalJsonError> {
    Ok(SubmittedEvidenceV1 {
        receipt_digest: canonical_json_digest(&submission.receipt)?,
        output_digest: canonical_json_digest(&submission.output)?,
        telemetry_summary_digest: canonical_json_digest(&submission.telemetry_summary)?,
    })
}

fn has_supported_contract_shape(
    contract: &pitgun_contract::DeterministicRunContractV1,
    input: &RunRaceInput,
) -> bool {
    let Ok(scenario_id) = RACING_SCENARIO_ID.parse() else {
        return false;
    };
    let Ok(scenario_version) = RACING_SCENARIO_VERSION.parse() else {
        return false;
    };
    let Some(expected_clock) = expected_clock(input) else {
        return false;
    };

    contract.contract_version == ContractVersion::V1
        && contract.scenario.id == scenario_id
        && contract.scenario.version == scenario_version
        && contract.runtime_profile == RuntimeProfile::PortableExactV1
        && contract.random.algorithm == RandomAlgorithm::PitgunSplitMix64V1
        && contract.random.stream_derivation == StreamDerivation::Sha256LabelV1
        && contract.clock == expected_clock
        && contract.event_ordering == EventOrderingV1::v1()
        && contract.input.media_type == InputMediaType::ApplicationJson
        && contract.input.canonicalization == InputCanonicalization::JcsRfc8785
}

fn has_supported_dynamic_contract_shape(
    contract: &pitgun_contract::DeterministicRunContractV1,
    input: &RunRaceInput,
) -> bool {
    let Ok(scenario_id) = RACING_DYNAMIC_SCENARIO_ID.parse() else {
        return false;
    };
    has_supported_contract_semantics(contract, input)
        && contract.scenario.id == scenario_id
        && contract.scenario.version
            == RACING_SCENARIO_VERSION
                .parse()
                .expect("static scenario version")
}

fn has_supported_contract_semantics(
    contract: &pitgun_contract::DeterministicRunContractV1,
    input: &RunRaceInput,
) -> bool {
    let Some(expected_clock) = expected_clock(input) else {
        return false;
    };
    contract.contract_version == ContractVersion::V1
        && contract.runtime_profile == RuntimeProfile::PortableExactV1
        && contract.random.algorithm == RandomAlgorithm::PitgunSplitMix64V1
        && contract.random.stream_derivation == StreamDerivation::Sha256LabelV1
        && contract.clock == expected_clock
        && contract.event_ordering == EventOrderingV1::v1()
        && contract.input.media_type == InputMediaType::ApplicationJson
        && contract.input.canonicalization == InputCanonicalization::JcsRfc8785
}

fn expected_clock(input: &RunRaceInput) -> Option<LogicalClockV1> {
    let hz = input.hz;
    if !hz.is_finite() || hz <= 0.0 || hz.fract() != 0.0 || hz > 1_000_000.0 {
        return None;
    }
    let hz = hz as u64;
    let divisor = greatest_common_divisor(1_000_000, hz);
    LogicalClockV1::new(0, 1_000_000 / divisor, hz / divisor).ok()
}

const fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn authorization_reason(error: &AuthorizationVerificationError) -> VerificationReasonCode {
    match error {
        AuthorizationVerificationError::Authorization(RunAuthorizationError::RunIdMismatch {
            ..
        }) => VerificationReasonCode::RunIdMismatch,
        AuthorizationVerificationError::Authorization(
            RunAuthorizationError::Expired
            | RunAuthorizationError::SubmissionGraceExpired
            | RunAuthorizationError::NotYetValid,
        ) => VerificationReasonCode::AuthorizationExpired,
        AuthorizationVerificationError::AttemptAuthorization(
            RunAttemptAuthorizationError::Authorization(
                RunAuthorizationError::Expired
                | RunAuthorizationError::SubmissionGraceExpired
                | RunAuthorizationError::NotYetValid,
            ),
        ) => VerificationReasonCode::AuthorizationExpired,
        AuthorizationVerificationError::AttemptAuthorization(
            RunAttemptAuthorizationError::InitialRunIdMismatch { .. },
        ) => VerificationReasonCode::RunIdMismatch,
        _ => VerificationReasonCode::InvalidAuthorization,
    }
}

fn replay_reason(
    error: &LinkedWorkloadError<pitgun_racing_simulator::RacingWorkloadError>,
) -> VerificationReasonCode {
    match error {
        LinkedWorkloadError::ModelMismatch { .. } => VerificationReasonCode::UnknownModel,
        LinkedWorkloadError::InputDigestMismatch { .. } => {
            VerificationReasonCode::ArtifactDigestMismatch
        }
        LinkedWorkloadError::Canonicalization(_) | LinkedWorkloadError::Workload(_) => {
            VerificationReasonCode::ReplayMismatch
        }
    }
}

/// Internal failure to calculate or construct a versioned verdict.
#[derive(Debug)]
pub enum RacingVerifierError {
    /// Canonical evidence identity could not be calculated.
    CanonicalJson(CanonicalJsonError),
    /// The verifier attempted to construct an invalid verdict state.
    InvalidVerdict(VerificationVerdictError),
}

impl fmt::Display for RacingVerifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalJson(error) => write!(formatter, "canonical evidence failed: {error}"),
            Self::InvalidVerdict(error) => write!(formatter, "invalid verifier verdict: {error}"),
        }
    }
}

impl std::error::Error for RacingVerifierError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CanonicalJson(error) => Some(error),
            Self::InvalidVerdict(error) => Some(error),
        }
    }
}

impl From<CanonicalJsonError> for RacingVerifierError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

impl From<VerificationVerdictError> for RacingVerifierError {
    fn from(error: VerificationVerdictError) -> Self {
        Self::InvalidVerdict(error)
    }
}

#[cfg(test)]
mod tests {
    use pitgun_contract::{
        ArtifactIdentity, AuthorizationSignatureAlgorithm, AuthorizationValidityV1,
        ContractVersion, DeterministicRunContractV1, Digest, EventOrderingV1, ExecutionId,
        Identifier, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
        RandomAlgorithm, RandomContractV1, RunAttemptAuthorizationV1,
        RunAttemptAuthorizationVersion, RunAuthorizationV1, RunAuthorizationVersion,
        RuntimeIdentity, RuntimeProfile, ScenarioIdentity, Seed, SemanticVersion,
        SignedRunAttemptAuthorizationV1, SignedRunAuthorizationV1, StreamDerivation,
        TelemetryFrame, TelemetrySummaryV1, VerificationReasonCode, VerificationStatus,
        canonical_json_digest,
    };
    use pitgun_racing_contract::{
        RacingCompletedDriverInstructionHistoryV1, RacingCompletedRunInputV1,
        RacingCompletedRunInputVersion, RacingDriverInstructionAuthorizationV1,
        RacingDriverInstructionAuthorizationVersion, RacingDriverInstructionBoundaryV1,
        RacingDriverInstructionEventV1, RacingDriverInstructionTimelineV1,
        RacingDriverInstructionTimelineVersion, RacingDrivingMode,
    };
    use pitgun_racing_simulator::evidence::{
        RacingDynamicExecutionRequestV1, RacingDynamicExecutionRequestVersion,
        RacingExecutionResolutionVersion, RacingHostedExecutionRequestV1,
        RacingHostedExecutionRequestVersion,
    };
    use pitgun_racing_simulator::{
        RacingCatalogSnapshot, RunRaceInput, RunRaceRequest, execute_authorized_dynamic_race,
        execute_authorized_race, get_circuit_with_catalog, racing_model_identity_for_version,
        racing_model_v1_identity, racing_model_v3_timeline_candidate_identity,
    };
    use pitgun_signing::{SigningKey, VerificationKeyring};

    use super::{
        RacingDynamicVerificationSubmissionV1, RacingVerificationSubmissionV1, RacingVerifier,
    };

    const NOW_MS: i64 = 1_722_345_678_901;
    const SECRET: &[u8] = b"pitgun-verifier-unit-test-secret";
    const KEY_ID: &str = "test-2026-07";

    struct Fixture {
        verifier: RacingVerifier,
        submission: RacingVerificationSubmissionV1,
        signing_key: SigningKey,
        catalog: RacingCatalogSnapshot,
        policy: ArtifactIdentity,
        verifier_identity: ArtifactIdentity,
    }

    struct DynamicFixture {
        verifier: RacingVerifier,
        submission: RacingDynamicVerificationSubmissionV1,
        signing_key: SigningKey,
        catalog: RacingCatalogSnapshot,
        policy: ArtifactIdentity,
        verifier_identity: ArtifactIdentity,
    }

    fn artifact(id: &str, bytes: &[u8]) -> ArtifactIdentity {
        ArtifactIdentity {
            id: id.parse().expect("artifact id"),
            version: SemanticVersion::new("1.0.0").expect("artifact version"),
            digest: Digest::from_bytes(bytes),
        }
    }

    fn runtime(engine: &str, target: &str, bytes: &[u8]) -> RuntimeIdentity {
        RuntimeIdentity {
            engine: engine.parse().expect("runtime engine"),
            engine_version: SemanticVersion::new("1.0.0").expect("runtime version"),
            target: target.parse().expect("runtime target"),
            artifact_digest: Digest::from_bytes(bytes),
        }
    }

    fn fixture() -> Fixture {
        fixture_for("1.0.0")
    }

    fn fixture_for(model_version: &str) -> Fixture {
        fixture_for_catalog(model_version, None)
    }

    fn fixture_for_catalog(model_version: &str, catalog_version: Option<&str>) -> Fixture {
        let document: serde_json::Value = serde_json::from_str(include_str!(
            "../../../apps/pitgun-cli/scenarios/racing-demo-v1.json"
        ))
        .expect("Racing input fixture");
        let input: RunRaceInput =
            serde_json::from_value(document["request"].clone()).expect("RunRaceInput");
        let catalog = if let Some(catalog_version) = catalog_version {
            let catalog_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../catalogs/racing")
                .join(catalog_version);
            RacingCatalogSnapshot::from_release_dir(catalog_path).expect("retained catalog release")
        } else if model_version == "0.10.0" {
            RacingCatalogSnapshot::embedded_model_v3_thermal().expect("embedded V3 thermal catalog")
        } else if model_version == "2.0.0" {
            RacingCatalogSnapshot::embedded_model_v2().expect("embedded V2 catalog")
        } else {
            RacingCatalogSnapshot::embedded().expect("embedded V1 catalog")
        };
        let model = racing_model_identity_for_version(model_version).expect("supported model");
        let policy = ArtifactIdentity {
            id: "pitgun.racing.tuning".parse().expect("policy id"),
            version: "1.0.0".parse().expect("policy version"),
            digest: Digest::from_bytes(include_bytes!("../../../policies/gametuning.v1.yaml")),
        };
        let verifier_identity = artifact("pitgun.verifier", b"verifier-v1-test-binary");
        let contract = DeterministicRunContractV1 {
            contract_version: ContractVersion::V1,
            scenario: ScenarioIdentity {
                id: "racing.race".parse().expect("scenario id"),
                version: "1.0.0".parse().expect("scenario version"),
            },
            model: model.clone(),
            data_pack: catalog.manifest().simulation_pack.identity.clone(),
            runtime_profile: RuntimeProfile::PortableExactV1,
            random: RandomContractV1 {
                seed: Seed::new(42),
                algorithm: RandomAlgorithm::PitgunSplitMix64V1,
                stream_derivation: StreamDerivation::Sha256LabelV1,
            },
            clock: LogicalClockV1::new(0, 200_000, 1).expect("clock"),
            event_ordering: EventOrderingV1::v1(),
            input: InputIdentity {
                media_type: InputMediaType::ApplicationJson,
                canonicalization: InputCanonicalization::JcsRfc8785,
                digest: canonical_json_digest(&input).expect("input digest"),
            },
        };
        let run_id = contract.run_id().expect("run id");
        let authorization = RunAuthorizationV1 {
            authorization_version: RunAuthorizationVersion::V1,
            nonce: Digest::from_bytes(b"nonce"),
            subject: "career.test".parse().expect("subject"),
            audience: "pitgun.verifier".parse().expect("audience"),
            contract: contract.clone(),
            run_id,
            policy: policy.clone(),
            signing_key_id: KEY_ID.parse().expect("key id"),
            validity: AuthorizationValidityV1 {
                issued_at_ms: NOW_MS - 60_000,
                expires_at_ms: NOW_MS + 60_000,
                late_submission_grace_ms: 120_000,
            },
        };
        let signing_key = SigningKey::from_secret(SECRET).expect("signing key");
        let signature = signing_key.sign(&authorization.signing_bytes().expect("signing bytes"));
        let signed_authorization = SignedRunAuthorizationV1 {
            authorization,
            algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
            signature,
        };

        let execution_id: ExecutionId = "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
            .parse()
            .expect("execution id");
        let submission = execute_authorized_race(
            RacingHostedExecutionRequestV1 {
                schema_version: RacingHostedExecutionRequestVersion::V1,
                signed_authorization,
                input,
                execution_id,
                wasm_artifact_digest: Digest::from_bytes(b"wasm"),
            },
            &catalog,
        )
        .expect("hosted WASM execution");

        let verifier = RacingVerifier::new(
            retained_keyring(&signing_key),
            "pitgun.verifier".parse().expect("audience"),
            model,
            policy.clone(),
            Some(catalog.clone()),
            verifier_identity.clone(),
        );

        Fixture {
            verifier,
            submission,
            signing_key,
            catalog,
            policy,
            verifier_identity,
        }
    }

    fn dynamic_fixture() -> DynamicFixture {
        let mut input = serde_json::from_str::<RunRaceRequest>(include_str!(
            "../../../crates/pitgun-racing-simulator/tests/golden/racing_run_v2.input.json"
        ))
        .expect("dynamic Racing input fixture")
        .input;
        input.race.laps = 3;
        input.race.competitors[0].driver_id = Some("balanced_reference".to_string());
        let catalog_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../catalogs/racing/v1.8.0");
        let catalog =
            RacingCatalogSnapshot::from_release_dir(catalog_path).expect("dynamic catalog");
        let model = racing_model_v3_timeline_candidate_identity();
        let policy = ArtifactIdentity {
            id: "pitgun.racing.tuning".parse().expect("policy id"),
            version: "1.0.0".parse().expect("policy version"),
            digest: Digest::from_bytes(include_bytes!("../../../policies/gametuning.v1.yaml")),
        };
        let initial_contract = DeterministicRunContractV1 {
            contract_version: ContractVersion::V1,
            scenario: ScenarioIdentity {
                id: "racing.dynamic-session".parse().expect("scenario id"),
                version: "1.0.0".parse().expect("scenario version"),
            },
            model: model.clone(),
            data_pack: catalog.manifest().simulation_pack.identity.clone(),
            runtime_profile: RuntimeProfile::PortableExactV1,
            random: RandomContractV1 {
                seed: Seed::new(376),
                algorithm: RandomAlgorithm::PitgunSplitMix64V1,
                stream_derivation: StreamDerivation::Sha256LabelV1,
            },
            clock: LogicalClockV1::new(0, 50_000, 1).expect("clock"),
            event_ordering: EventOrderingV1::v1(),
            input: InputIdentity {
                media_type: InputMediaType::ApplicationJson,
                canonicalization: InputCanonicalization::JcsRfc8785,
                digest: canonical_json_digest(&input).expect("input digest"),
            },
        };
        let instruction_profile = catalog
            .driver_instruction_profile()
            .expect("instruction profile");
        let instruction_profile_identity = catalog
            .driver_instruction_profile_identity()
            .expect("instruction profile identity")
            .clone();
        let segment_count = u32::try_from(
            get_circuit_with_catalog(&catalog, &input.race.track_id)
                .expect("circuit")
                .s_m
                .len(),
        )
        .expect("segment count");
        let decision_envelope = RacingDriverInstructionAuthorizationV1 {
            schema_version: RacingDriverInstructionAuthorizationVersion::V1,
            authorized_input: initial_contract.input.clone(),
            instruction_profile: instruction_profile_identity.clone(),
            allowed_modes: vec![
                RacingDrivingMode::Manage,
                RacingDrivingMode::Balanced,
                RacingDrivingMode::Attack,
            ],
            boundary_granularity: instruction_profile.boundary_granularity,
            max_events_per_session: instruction_profile.max_events_per_session,
            competitor_ids: vec!["player".to_string()],
            lap_count: input.race.laps,
            segment_count,
        };
        let authorization = RunAttemptAuthorizationV1 {
            authorization_version: RunAttemptAuthorizationVersion::V1,
            nonce: Digest::from_bytes(b"dynamic-verifier-nonce"),
            execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a597"
                .parse()
                .expect("execution id"),
            subject: "career.dynamic-verifier".parse().expect("subject"),
            audience: "pitgun.verifier".parse().expect("audience"),
            initial_run_id: initial_contract.run_id().expect("initial run id"),
            initial_contract,
            decision_envelope: decision_envelope
                .artifact_identity()
                .expect("decision envelope identity"),
            policy: policy.clone(),
            signing_key_id: KEY_ID.parse().expect("key id"),
            validity: AuthorizationValidityV1 {
                issued_at_ms: NOW_MS - 60_000,
                expires_at_ms: NOW_MS + 60_000,
                late_submission_grace_ms: 120_000,
            },
        };
        let signing_key = SigningKey::from_secret(SECRET).expect("signing key");
        let signature = signing_key.sign(&authorization.signing_bytes().expect("signing bytes"));
        let completed_input = RacingCompletedRunInputV1 {
            schema_version: RacingCompletedRunInputVersion::V1,
            authorized_input: authorization.initial_contract.input.clone(),
            driver_instructions: RacingCompletedDriverInstructionHistoryV1 {
                instruction_profile: instruction_profile_identity,
                initial_mode: RacingDrivingMode::Balanced,
                competitor_ids: vec!["player".to_string()],
                applied_timeline: RacingDriverInstructionTimelineV1 {
                    schema_version: RacingDriverInstructionTimelineVersion::V1,
                    events: vec![
                        RacingDriverInstructionEventV1 {
                            sequence: 0,
                            competitor_id: "player".to_string(),
                            effective_at: RacingDriverInstructionBoundaryV1 {
                                lap_index: 1,
                                segment_index: 0,
                            },
                            mode: RacingDrivingMode::Attack,
                        },
                        RacingDriverInstructionEventV1 {
                            sequence: 1,
                            competitor_id: "player".to_string(),
                            effective_at: RacingDriverInstructionBoundaryV1 {
                                lap_index: 2,
                                segment_index: 0,
                            },
                            mode: RacingDrivingMode::Balanced,
                        },
                    ],
                },
            },
        };
        let submission = execute_authorized_dynamic_race(
            RacingDynamicExecutionRequestV1 {
                schema_version: RacingDynamicExecutionRequestVersion::V1,
                signed_authorization: SignedRunAttemptAuthorizationV1 {
                    authorization,
                    algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
                    signature,
                },
                decision_envelope,
                catalog_release: catalog.release_identity().clone(),
                input,
                completed_input,
                wasm_artifact_digest: Digest::from_bytes(b"dynamic verifier wasm"),
            },
            &catalog,
        )
        .expect("dynamic execution");
        let verifier_identity = artifact("pitgun.verifier", b"verifier-v1-test-binary");
        let verifier = RacingVerifier::new(
            retained_keyring(&signing_key),
            "pitgun.verifier".parse().expect("audience"),
            model,
            policy.clone(),
            Some(catalog.clone()),
            verifier_identity.clone(),
        );

        DynamicFixture {
            verifier,
            submission,
            signing_key,
            catalog,
            policy,
            verifier_identity,
        }
    }

    fn resign(submission: &mut RacingVerificationSubmissionV1, key: &SigningKey) {
        submission.signed_authorization.authorization.run_id = submission
            .signed_authorization
            .authorization
            .contract
            .run_id()
            .expect("changed run id");
        submission.signed_authorization.signature = key.sign(
            &submission
                .signed_authorization
                .authorization
                .signing_bytes()
                .expect("changed signing bytes"),
        );
    }

    fn resign_dynamic(submission: &mut RacingDynamicVerificationSubmissionV1, key: &SigningKey) {
        submission.signed_authorization.authorization.initial_run_id = submission
            .signed_authorization
            .authorization
            .initial_contract
            .run_id()
            .expect("changed initial run id");
        submission.signed_authorization.signature = key.sign(
            &submission
                .signed_authorization
                .authorization
                .signing_bytes()
                .expect("changed attempt signing bytes"),
        );
    }

    fn reason(
        verifier: &RacingVerifier,
        submission: &RacingVerificationSubmissionV1,
    ) -> (VerificationStatus, Option<VerificationReasonCode>) {
        let verdict = verifier
            .verify(submission, NOW_MS)
            .expect("verdict construction");
        (verdict.status, verdict.reason_code)
    }

    fn dynamic_reason(
        verifier: &RacingVerifier,
        submission: &RacingDynamicVerificationSubmissionV1,
    ) -> (VerificationStatus, Option<VerificationReasonCode>) {
        let verdict = verifier
            .verify_attempt(submission, NOW_MS)
            .expect("dynamic verdict construction");
        (verdict.status, verdict.reason_code)
    }

    fn retained_keyring(key: &SigningKey) -> VerificationKeyring {
        let mut keyring = VerificationKeyring::new();
        keyring.insert(KEY_ID.parse::<Identifier>().expect("key id"), key.clone());
        keyring
    }

    #[test]
    fn valid_catalog_backed_submission_is_verified() {
        let fixture = fixture();

        let verdict = fixture
            .verifier
            .verify(&fixture.submission, NOW_MS)
            .expect("verified verdict");

        assert_eq!(verdict.status, VerificationStatus::Verified);
        assert_eq!(verdict.reason_code, None);
        assert_eq!(
            verdict.run_id,
            fixture.submission.signed_authorization.authorization.run_id
        );
        assert_eq!(
            verdict.execution_id,
            fixture.submission.receipt.receipt.execution_id
        );
        assert_eq!(
            verdict
                .verified_resolution
                .expect("verified resolution")
                .data_pack,
            fixture.catalog.manifest().simulation_pack.identity
        );
    }

    #[test]
    fn valid_v2_catalog_backed_submission_is_verified() {
        let fixture = fixture_for("2.0.0");

        let verdict = fixture
            .verifier
            .verify(&fixture.submission, NOW_MS)
            .expect("verified V2 verdict");

        assert_eq!(verdict.status, VerificationStatus::Verified);
        assert_eq!(
            verdict
                .verified_resolution
                .expect("verified resolution")
                .model
                .version
                .to_string(),
            "2.0.0"
        );
    }

    #[test]
    fn parameter_backed_submission_records_and_verifies_exact_resolution() {
        let fixture = fixture_for_catalog("2.0.0", Some("v1.4.0"));
        let expected_parameters = fixture
            .catalog
            .model_parameters_identity()
            .expect("model-parameter identity");
        let resolution = fixture
            .submission
            .execution_resolution
            .as_ref()
            .expect("explicit execution resolution");

        assert_eq!(
            resolution.catalog_release,
            *fixture.catalog.release_identity()
        );
        assert_eq!(
            resolution.simulation_pack,
            fixture.catalog.manifest().simulation_pack.identity
        );
        assert_eq!(
            resolution.model_parameters.as_ref(),
            Some(expected_parameters)
        );
        assert!(resolution.thermal_family_profile.is_none());
        assert_eq!(
            reason(&fixture.verifier, &fixture.submission),
            (VerificationStatus::Verified, None)
        );

        let mut missing = fixture.submission.clone();
        missing.execution_resolution = None;
        assert_eq!(
            reason(&fixture.verifier, &missing),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::ArtifactDigestMismatch)
            )
        );

        let mut substituted = fixture.submission.clone();
        substituted
            .execution_resolution
            .as_mut()
            .expect("resolution")
            .model_parameters
            .as_mut()
            .expect("model parameters")
            .digest = Digest::from_bytes(b"substituted-parameter-resource");
        assert_eq!(
            reason(&fixture.verifier, &substituted),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::ArtifactDigestMismatch)
            )
        );
    }

    #[test]
    fn thermal_candidate_submission_records_and_verifies_exact_resolution() {
        let fixture = fixture_for_catalog("0.10.0", Some("v1.5.0"));
        let expected_profile = fixture
            .catalog
            .thermal_family_profile_identity()
            .expect("thermal-family profile");
        let resolution = fixture
            .submission
            .execution_resolution
            .as_ref()
            .expect("V3 thermal execution resolution");

        assert_eq!(
            resolution.schema_version,
            RacingExecutionResolutionVersion::V2
        );
        assert_eq!(
            resolution.model,
            racing_model_identity_for_version("0.10.0").unwrap()
        );
        assert!(resolution.model_parameters.is_none());
        assert_eq!(
            resolution.thermal_family_profile.as_ref(),
            Some(expected_profile)
        );
        assert_eq!(
            reason(&fixture.verifier, &fixture.submission),
            (VerificationStatus::Verified, None)
        );

        let mut substituted = fixture.submission.clone();
        substituted
            .execution_resolution
            .as_mut()
            .expect("resolution")
            .thermal_family_profile
            .as_mut()
            .expect("thermal-family profile")
            .digest = Digest::from_bytes(b"substituted-thermal-family-profile");
        assert_eq!(
            reason(&fixture.verifier, &substituted),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::ArtifactDigestMismatch)
            )
        );
    }

    #[test]
    fn missing_retained_catalog_is_pending() {
        let fixture = fixture();
        let verifier = RacingVerifier::new(
            retained_keyring(&fixture.signing_key),
            "pitgun.verifier".parse().expect("audience"),
            racing_model_v1_identity(),
            fixture.policy,
            None,
            fixture.verifier_identity,
        );

        assert_eq!(
            reason(&verifier, &fixture.submission),
            (
                VerificationStatus::Pending,
                Some(VerificationReasonCode::CatalogUnavailable)
            )
        );
    }

    #[test]
    fn mutated_signature_and_contract_are_rejected() {
        let fixture = fixture();
        let mut bad_signature = fixture.submission.clone();
        bad_signature.signed_authorization.signature = "00".to_owned();
        assert_eq!(
            reason(&fixture.verifier, &bad_signature),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::InvalidAuthorization)
            )
        );

        let mut changed_contract = fixture.submission.clone();
        changed_contract
            .signed_authorization
            .authorization
            .contract
            .scenario
            .version = "2.0.0".parse().expect("changed scenario version");
        assert_eq!(
            reason(&fixture.verifier, &changed_contract),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::RunIdMismatch)
            )
        );

        resign(&mut changed_contract, &fixture.signing_key);
        assert_eq!(
            reason(&fixture.verifier, &changed_contract),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::InvalidAuthorization)
            )
        );

        let mut expired = fixture.submission.clone();
        expired
            .signed_authorization
            .authorization
            .validity
            .issued_at_ms = NOW_MS - 120_000;
        expired
            .signed_authorization
            .authorization
            .validity
            .expires_at_ms = NOW_MS - 60_000;
        expired
            .signed_authorization
            .authorization
            .validity
            .late_submission_grace_ms = 0;
        resign(&mut expired, &fixture.signing_key);
        assert_eq!(
            reason(&fixture.verifier, &expired),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::AuthorizationExpired)
            )
        );
    }

    #[test]
    fn mutated_input_receipt_output_and_telemetry_are_rejected() {
        let fixture = fixture();

        let mut changed_input = fixture.submission.clone();
        changed_input.input.race.laps += 1;
        assert_eq!(
            reason(&fixture.verifier, &changed_input),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::ArtifactDigestMismatch)
            )
        );

        let mut changed_receipt = fixture.submission.clone();
        changed_receipt.receipt.receipt.run_id = Digest::from_bytes(b"another run");
        assert_eq!(
            reason(&fixture.verifier, &changed_receipt),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::ReceiptMismatch)
            )
        );

        let mut changed_output = fixture.submission.clone();
        changed_output.output.player_lap_times_ms[0] += 1;
        assert_eq!(
            reason(&fixture.verifier, &changed_output),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::OutputMismatch)
            )
        );

        let mut changed_telemetry = fixture.submission.clone();
        let no_frames = Vec::<TelemetryFrame>::new();
        changed_telemetry.telemetry_summary =
            TelemetrySummaryV1::from_ordered_frames(0, no_frames.iter(), 0)
                .expect("empty telemetry summary");
        assert_eq!(
            reason(&fixture.verifier, &changed_telemetry),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::TelemetryMismatch)
            )
        );
    }

    #[test]
    fn unknown_model_data_pack_and_policy_fail_closed() {
        let fixture = fixture();

        let mut unknown_model = fixture.submission.clone();
        unknown_model
            .signed_authorization
            .authorization
            .contract
            .model = artifact("pitgun.racing.other", b"unknown model");
        resign(&mut unknown_model, &fixture.signing_key);
        assert_eq!(
            reason(&fixture.verifier, &unknown_model),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::UnknownModel)
            )
        );

        let mut unknown_pack = fixture.submission.clone();
        unknown_pack
            .signed_authorization
            .authorization
            .contract
            .data_pack = artifact("pitgun.racing.simulation", b"unknown pack");
        resign(&mut unknown_pack, &fixture.signing_key);
        assert_eq!(
            reason(&fixture.verifier, &unknown_pack),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::UnknownDataPack)
            )
        );

        let mut unknown_policy = fixture.submission.clone();
        unknown_policy.signed_authorization.authorization.policy =
            artifact("pitgun.racing.tuning", b"unknown policy");
        resign(&mut unknown_policy, &fixture.signing_key);
        assert_eq!(
            reason(&fixture.verifier, &unknown_policy),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::UnknownPolicy)
            )
        );
    }

    #[test]
    fn native_and_wasm_attempts_share_run_id_but_not_execution_id() {
        let fixture = fixture();
        let first = fixture
            .verifier
            .verify(&fixture.submission, NOW_MS)
            .expect("WASM verdict");

        let mut native = fixture.submission.clone();
        let native_execution_id: ExecutionId = "018f3b78-7e9a-7d20-a5e1-4ed92f02a592"
            .parse()
            .expect("native execution id");
        native.receipt.receipt = pitgun_contract::ExecutionReceiptV1::for_contract(
            &native.signed_authorization.authorization.contract,
            native_execution_id,
            runtime("pitgun-native", "aarch64-macos", b"native"),
            canonical_json_digest(&native.output).expect("output digest"),
            canonical_json_digest(&native.telemetry_summary).expect("telemetry digest"),
        )
        .expect("native receipt");
        let second = fixture
            .verifier
            .verify(&native, NOW_MS)
            .expect("native verdict");

        assert_eq!(first.status, VerificationStatus::Verified);
        assert_eq!(second.status, VerificationStatus::Verified);
        assert_eq!(first.run_id, second.run_id);
        assert_ne!(first.execution_id, second.execution_id);
    }

    #[test]
    fn valid_dynamic_attempt_is_replayed_with_its_final_run_id() {
        let fixture = dynamic_fixture();

        let verdict = fixture
            .verifier
            .verify_attempt(&fixture.submission, NOW_MS)
            .expect("dynamic verdict");

        assert_eq!(verdict.status, VerificationStatus::Verified);
        assert_eq!(verdict.reason_code, None);
        assert_eq!(verdict.run_id, fixture.submission.receipt.receipt.run_id);
        assert_ne!(
            verdict.run_id,
            fixture
                .submission
                .signed_authorization
                .authorization
                .initial_run_id
        );
        assert_eq!(
            verdict
                .verified_resolution
                .expect("verified resolution")
                .data_pack,
            fixture.catalog.manifest().simulation_pack.identity
        );
    }

    #[test]
    fn dynamic_attempt_fails_closed_without_retained_catalog() {
        let fixture = dynamic_fixture();
        let verifier = RacingVerifier::new(
            retained_keyring(&fixture.signing_key),
            "pitgun.verifier".parse().expect("audience"),
            racing_model_v3_timeline_candidate_identity(),
            fixture.policy,
            None,
            fixture.verifier_identity,
        );

        assert_eq!(
            dynamic_reason(&verifier, &fixture.submission),
            (
                VerificationStatus::Pending,
                Some(VerificationReasonCode::CatalogUnavailable)
            )
        );
    }

    #[test]
    fn dynamic_attempt_rejects_signature_and_timeline_mutations() {
        let fixture = dynamic_fixture();

        let mut bad_signature = fixture.submission.clone();
        bad_signature.signed_authorization.signature = "00".to_string();
        assert_eq!(
            dynamic_reason(&fixture.verifier, &bad_signature),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::InvalidAuthorization)
            )
        );

        let mut expired = fixture.submission.clone();
        expired
            .signed_authorization
            .authorization
            .validity
            .issued_at_ms = NOW_MS - 120_000;
        expired
            .signed_authorization
            .authorization
            .validity
            .expires_at_ms = NOW_MS - 60_000;
        expired
            .signed_authorization
            .authorization
            .validity
            .late_submission_grace_ms = 0;
        resign_dynamic(&mut expired, &fixture.signing_key);
        assert_eq!(
            dynamic_reason(&fixture.verifier, &expired),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::AuthorizationExpired)
            )
        );

        let mut changed_envelope = fixture.submission.clone();
        changed_envelope.decision_envelope.allowed_modes.remove(0);
        assert_eq!(
            dynamic_reason(&fixture.verifier, &changed_envelope),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::InvalidAuthorization)
            )
        );

        let mut missing = fixture.submission.clone();
        missing
            .completed_input
            .driver_instructions
            .applied_timeline
            .events
            .remove(0);
        assert_eq!(
            dynamic_reason(&fixture.verifier, &missing),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::InvalidAuthorization)
            )
        );

        let mut reordered = fixture.submission.clone();
        reordered
            .completed_input
            .driver_instructions
            .applied_timeline
            .events
            .swap(0, 1);
        assert_eq!(
            dynamic_reason(&fixture.verifier, &reordered),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::InvalidAuthorization)
            )
        );

        let mut inserted = fixture.submission.clone();
        inserted
            .completed_input
            .driver_instructions
            .applied_timeline
            .events
            .push(RacingDriverInstructionEventV1 {
                sequence: 2,
                competitor_id: "player".to_string(),
                effective_at: RacingDriverInstructionBoundaryV1 {
                    lap_index: inserted.input.race.laps,
                    segment_index: 0,
                },
                mode: RacingDrivingMode::Attack,
            });
        assert_eq!(
            dynamic_reason(&fixture.verifier, &inserted),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::InvalidAuthorization)
            )
        );

        let mut altered = fixture.submission.clone();
        altered
            .completed_input
            .driver_instructions
            .applied_timeline
            .events[0]
            .mode = RacingDrivingMode::Manage;
        assert_eq!(
            dynamic_reason(&fixture.verifier, &altered),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::ReceiptMismatch)
            )
        );
    }

    #[test]
    fn dynamic_attempt_rejects_physically_altered_output() {
        let fixture = dynamic_fixture();
        let mut altered = fixture.submission.clone();
        altered.output.player_lap_times_ms[0] += 1;

        assert_eq!(
            dynamic_reason(&fixture.verifier, &altered),
            (
                VerificationStatus::Rejected,
                Some(VerificationReasonCode::OutputMismatch)
            )
        );
    }
}

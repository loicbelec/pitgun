//! Signed service-control adapter. Reuses the existing V9 thermal Solver and
//! generic signed-attempt lifecycle, without authorizing provider identities.
use crate::control_prototype::{
    AppliedDecision, ControlStep, LocalControlConfig, LocalControlledRacingSession,
};
use crate::evidence::{RacingExecutionResolutionV1, RacingOutputV1, RacingRunEvidenceV1};
use crate::{
    EmbeddedCatalog, IncrementalExecutionStreamEventV1, RaceOutput, RacingCatalogSnapshot,
    RunRaceInput, RunRaceRequest, racing_model_v3_component_candidate_identity,
};
use pitgun_contract::{
    ArtifactIdentity, CatalogReleaseIdentityV1, DeterministicRunContractV1, Digest,
    RunBundleReceiptV1, RunBundleReceiptVersion, RuntimeIdentity, SignedRunAttemptAuthorizationV1,
    canonical_json_digest,
};
use pitgun_racing_contract::{
    RacingCompletedServiceInputV1, RacingDriverInstructionTimelineV1,
    RacingDriverInstructionTimelineVersion, RacingServiceAuthorizationV1,
    RacingServiceAuthorizationVersion,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingServiceAuthorizationResponseV1 {
    pub signed: SignedRunAttemptAuthorizationV1,
    pub canonical_input: RunRaceInput,
    pub decision_envelope: RacingServiceAuthorizationV1,
    pub catalog_release: CatalogReleaseIdentityV1,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingServiceDecisionV1 {
    pub observation_digest: Digest,
    pub applied: AppliedDecision,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingServiceSubmissionVersion {
    #[serde(rename = "pitgun.racing-service-verification-submission/v1")]
    V1,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingServiceVerificationSubmissionV1 {
    pub schema_version: RacingServiceSubmissionVersion,
    pub signed_authorization: SignedRunAttemptAuthorizationV1,
    pub decision_envelope: RacingServiceAuthorizationV1,
    pub catalog_release: CatalogReleaseIdentityV1,
    pub input: RunRaceInput,
    pub completed_input: RacingCompletedServiceInputV1,
    pub applied_history: Vec<RacingServiceDecisionV1>,
    pub receipt: RunBundleReceiptV1,
    pub output: RacingOutputV1,
    pub telemetry_summary: pitgun_contract::TelemetrySummaryV1,
    pub execution_resolution: RacingExecutionResolutionV1,
}

/// This exact template is the content-addressed policy. Run-specific fields
/// (execution/runtime/tyres) are separately bound in the signed envelope.
fn policy_template() -> serde_json::Value {
    serde_json::json!({
        "physics":"pitgun.local-career-power-unit-thermal/v1",
        "decision_laps":[],
        "service_windows":{"schema_version":"pitgun.racing-service-window/v1","closing_margin_us":5000000},
        "event_policy":{"schema_version":"pitgun.racing-rival-pit-policy/v1","max_pre_stop_gap_us":8000000,
            "max_event_age_us":120000000,"planned_stop_within_laps":2,"cooldown_laps":2,"max_windows":3},
        "planned_stop_reviews":{"schema_version":"pitgun.racing-planned-stop-review/v1"},
        "strategy_observation":{"schema_version":"pitgun.racing-strategy-observation/v1"}
    })
}
pub fn service_control_policy_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-service-control-policy".parse().unwrap(),
        version: "1.0.0".parse().unwrap(),
        digest: canonical_json_digest(&policy_template()).expect("finite static policy"),
    }
}
pub fn service_control_config(
    envelope: &RacingServiceAuthorizationV1,
    execution: pitgun_contract::ExecutionId,
) -> Result<LocalControlConfig, String> {
    let mut value = policy_template();
    value["execution_id"] = serde_json::json!(execution.to_string());
    value["runtime_digest"] = serde_json::json!(envelope.runtime_digest);
    value["allowed_tire_ids"] = serde_json::json!(envelope.allowed_tire_ids);
    serde_json::from_value(value).map_err(|e| e.to_string())
}
pub fn empty_timeline() -> RacingDriverInstructionTimelineV1 {
    RacingDriverInstructionTimelineV1 {
        schema_version: RacingDriverInstructionTimelineVersion::V1,
        events: vec![],
    }
}
pub fn service_runtime_identity(runtime_digest: Digest) -> RuntimeIdentity {
    RuntimeIdentity {
        engine: "pitgun-wasm".parse().expect("static runtime id"),
        engine_version: env!("CARGO_PKG_VERSION")
            .parse()
            .expect("static runtime version"),
        target: "wasm32-unknown-unknown"
            .parse()
            .expect("static WASM target"),
        artifact_digest: runtime_digest,
    }
}
pub fn resolve_service_authorization(
    initial: &DeterministicRunContractV1,
    input: &RunRaceInput,
    catalog: &RacingCatalogSnapshot,
    runtime_digest: Digest,
) -> Result<RacingServiceAuthorizationV1, String> {
    if initial.model != racing_model_v3_component_candidate_identity() {
        return Err("service control requires the exact component/V9 thermal model".into());
    }
    catalog
        .manifest()
        .validate_for_run(initial)
        .map_err(|e| e.to_string())?;
    if canonical_json_digest(input).map_err(|e| e.to_string())? != initial.input.digest
        || input.hz != 5.0
        || input.race.competitors.len() > 16
        || input
            .race
            .competitors
            .iter()
            .any(|c| c.id.is_empty() || c.id.len() > 64)
    {
        return Err("service input mismatch or limits exceeded (16 competitors, 5 Hz)".into());
    }
    let players: Vec<_> = input
        .race
        .competitors
        .iter()
        .filter(|c| c.is_player)
        .collect();
    if players.len() != 1 {
        return Err("service control requires exactly one player".into());
    }
    let mut tires: Vec<_> = EmbeddedCatalog::from_snapshot(catalog)?
        .tires
        .keys()
        .cloned()
        .collect();
    tires.sort();
    let envelope = RacingServiceAuthorizationV1 {
        schema_version: RacingServiceAuthorizationVersion::V1,
        authorized_input: initial.input.clone(),
        control_policy: service_control_policy_identity(),
        runtime_digest,
        player_id: players[0].id.clone(),
        allowed_tire_ids: tires,
        lap_count: input.race.laps,
        max_decisions: 35,
    };
    envelope.validate(initial, &service_control_policy_identity())?;
    Ok(envelope)
}
pub fn validate_service_start(
    response: &RacingServiceAuthorizationResponseV1,
    catalog: &RacingCatalogSnapshot,
) -> Result<LocalControlConfig, String> {
    let attempt = &response.signed.authorization;
    crate::validate_attempt_signature_shape(response.signed.algorithm, &response.signed.signature)?;
    response
        .decision_envelope
        .validate_attempt(attempt, &service_control_policy_identity())?;
    if response.catalog_release != *catalog.release_identity() {
        return Err("service catalog release mismatch".into());
    }
    let expected = resolve_service_authorization(
        &attempt.initial_contract,
        &response.canonical_input,
        catalog,
        response.decision_envelope.runtime_digest,
    )?;
    if expected != response.decision_envelope {
        return Err("service envelope differs from resolved permissions".into());
    }
    service_control_config(&expected, attempt.execution_id)
}
pub fn start_service_control(
    response: &RacingServiceAuthorizationResponseV1,
    catalog: &RacingCatalogSnapshot,
) -> Result<LocalControlledRacingSession, String> {
    let config = validate_service_start(response, catalog)?;
    LocalControlledRacingSession::start(
        RunRaceRequest {
            input: response.canonical_input.clone(),
            seed: response
                .signed
                .authorization
                .initial_contract
                .random
                .seed
                .get(),
            era: Some(response.canonical_input.era),
            hz: Some(response.canonical_input.hz),
        },
        catalog,
        empty_timeline(),
        config,
    )
}

/// Reconstructs every mandatory window, not only the caller's list of actions.
/// Missing, extra, reordered, altered observations or receipts fail closed.
pub fn replay_service_control(
    response: &RacingServiceAuthorizationResponseV1,
    catalog: &RacingCatalogSnapshot,
    history: &[RacingServiceDecisionV1],
) -> Result<RaceOutput, String> {
    if history.len() > usize::from(response.decision_envelope.max_decisions) {
        return Err("service history exceeds signed limit".into());
    }
    let mut session = start_service_control(response, catalog)?;
    let mut decisions = history.iter();
    loop {
        match session.advance()? {
            ControlStep::AwaitingDecision(observation) => {
                let record = decisions.next().ok_or("missing service decision")?;
                if observation.digest()? != record.observation_digest {
                    return Err("regenerated service observation differs".into());
                }
                if session.propose(record.applied.proposal.clone())? != record.applied {
                    return Err("regenerated service receipt differs".into());
                }
            }
            ControlStep::Batch(batch) => {
                for record in batch.records() {
                    if let IncrementalExecutionStreamEventV1::Complete(output) = record.event() {
                        if decisions.next().is_some() {
                            return Err("extra service decision".into());
                        }
                        return Ok(output.clone());
                    }
                }
            }
        }
    }
}

/// Compact evidence from the actual local output; cryptographic validation and
/// independent re-execution remain mandatory at Verifier, never in the browser.
pub fn service_control_evidence(
    response: RacingServiceAuthorizationResponseV1,
    catalog: &RacingCatalogSnapshot,
    history: Vec<RacingServiceDecisionV1>,
    output: &RaceOutput,
) -> Result<RacingServiceVerificationSubmissionV1, String> {
    validate_service_start(&response, catalog)?;
    if history.len() > usize::from(response.decision_envelope.max_decisions) {
        return Err("service history exceeds signed limit".into());
    }
    let history_digest = canonical_json_digest(&history).map_err(|e| e.to_string())?;
    let completed = response.decision_envelope.completed_input(history_digest)?;
    let contract = response.decision_envelope.final_contract(
        &response.signed.authorization,
        &completed,
        history_digest,
        &service_control_policy_identity(),
    )?;
    let evidence = RacingRunEvidenceV1::from_race_output(output).map_err(|e| e.to_string())?;
    let runtime = service_runtime_identity(response.decision_envelope.runtime_digest);
    let receipt = evidence
        .execution_receipt(
            &contract,
            response.signed.authorization.execution_id,
            runtime,
        )
        .map_err(|e| e.to_string())?;
    response
        .signed
        .authorization
        .validate_completed_receipt(&contract, &receipt)
        .map_err(|e| e.to_string())?;
    let resolution = RacingExecutionResolutionV1::from_catalog(catalog, &contract.model)
        .ok_or("service catalog lacks explicit resolution")?;
    Ok(RacingServiceVerificationSubmissionV1 {
        schema_version: RacingServiceSubmissionVersion::V1,
        signed_authorization: response.signed,
        decision_envelope: response.decision_envelope,
        catalog_release: response.catalog_release,
        input: response.canonical_input,
        completed_input: completed,
        applied_history: history,
        receipt: RunBundleReceiptV1 {
            schema_version: RunBundleReceiptVersion::V1,
            receipt,
        },
        output: evidence.output,
        telemetry_summary: evidence.telemetry_summary,
        execution_resolution: resolution,
    })
}

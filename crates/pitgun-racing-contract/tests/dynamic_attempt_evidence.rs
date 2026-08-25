use pitgun_contract::{
    ArtifactIdentity, AuthorizationSignatureAlgorithm, AuthorizationValidityV1, ContractVersion,
    DeterministicRunContractV1, Digest, EventOrderingV1, ExecutionReceiptV1, Identifier,
    InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1, RandomAlgorithm,
    RandomContractV1, RunAttemptAuthorizationV1, RunAttemptAuthorizationVersion, RuntimeIdentity,
    RuntimeProfile, ScenarioIdentity, Seed, SemanticVersion, SignedRunAttemptAuthorizationV1,
    StreamDerivation,
};
use pitgun_racing_contract::{
    RacingCompletedDriverInstructionHistoryV1, RacingCompletedRunInputV1,
    RacingCompletedRunInputVersion, RacingDriverContractError,
    RacingDriverInstructionAuthorizationV1, RacingDriverInstructionAuthorizationVersion,
    RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionBoundaryV1,
    RacingDriverInstructionEventV1, RacingDriverInstructionProfileV1,
    RacingDriverInstructionProfileVersion, RacingDriverInstructionTimelineV1,
    RacingDriverInstructionTimelineVersion, RacingDrivingMode, RacingInstructionAuthorizationError,
};
use pitgun_signing::{AuthorizationVerificationError, SigningKey, VerificationKeyring};
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const FIXTURE_JSON: &str = include_str!("fixtures/racing_dynamic_attempt_evidence_v1.json");
const FIXTURE_SECRET: &[u8] = b"racing-dynamic-attempt-golden-secret-v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RacingDynamicAttemptEvidenceFixtureV1 {
    fixture_version: String,
    instruction_profile_identity: ArtifactIdentity,
    instruction_profile: RacingDriverInstructionProfileV1,
    decision_envelope: RacingDriverInstructionAuthorizationV1,
    signed_attempt: SignedRunAttemptAuthorizationV1,
    completed_input: RacingCompletedRunInputV1,
    final_contract: DeterministicRunContractV1,
    receipt: ExecutionReceiptV1,
}

fn artifact(id: &str, version: &str, bytes: &[u8]) -> ArtifactIdentity {
    ArtifactIdentity {
        id: Identifier::new(id).expect("artifact id"),
        version: SemanticVersion::new(version).expect("artifact version"),
        digest: Digest::from_bytes(bytes),
    }
}

fn initial_contract() -> DeterministicRunContractV1 {
    DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: "racing.dynamic-session".parse().expect("scenario id"),
            version: "1.0.0".parse().expect("scenario version"),
        },
        model: artifact("pitgun.racing-model-v3", "0.13.0", b"dynamic model"),
        data_pack: artifact("pitgun.racing", "1.6.0", b"dynamic catalog"),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(363),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 50_000, 1).expect("logical clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: Digest::from_bytes(b"initial dynamic Racing input"),
        },
    }
}

fn instruction_profile_identity() -> ArtifactIdentity {
    artifact(
        "pitgun.racing-driver-instructions",
        "1.0.0",
        b"dynamic instruction profile",
    )
}

fn instruction_profile() -> RacingDriverInstructionProfileV1 {
    RacingDriverInstructionProfileV1 {
        schema_version: RacingDriverInstructionProfileVersion::V1,
        default_mode: RacingDrivingMode::Balanced,
        boundary_granularity: RacingDriverInstructionBoundaryGranularityV1::LapStart,
        max_events_per_session: 8,
    }
}

fn instruction_event(
    sequence: u32,
    competitor_id: &str,
    lap_index: u16,
    mode: RacingDrivingMode,
) -> RacingDriverInstructionEventV1 {
    RacingDriverInstructionEventV1 {
        sequence,
        competitor_id: competitor_id.to_string(),
        effective_at: RacingDriverInstructionBoundaryV1 {
            lap_index,
            segment_index: 0,
        },
        mode,
    }
}

fn generated_fixture() -> RacingDynamicAttemptEvidenceFixtureV1 {
    let initial_contract = initial_contract();
    let instruction_profile_identity = instruction_profile_identity();
    let instruction_profile = instruction_profile();
    let decision_envelope = RacingDriverInstructionAuthorizationV1 {
        schema_version: RacingDriverInstructionAuthorizationVersion::V1,
        authorized_input: initial_contract.input.clone(),
        instruction_profile: instruction_profile_identity.clone(),
        allowed_modes: vec![
            RacingDrivingMode::Manage,
            RacingDrivingMode::Balanced,
            RacingDrivingMode::Attack,
        ],
        boundary_granularity: RacingDriverInstructionBoundaryGranularityV1::LapStart,
        max_events_per_session: 4,
        competitor_ids: vec!["ai-01".to_string(), "player".to_string()],
        lap_count: 5,
        segment_count: 20,
    };
    let execution_id = "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
        .parse()
        .expect("execution id");
    let authorization = RunAttemptAuthorizationV1 {
        authorization_version: RunAttemptAuthorizationVersion::V1,
        nonce: Digest::from_bytes(b"dynamic attempt nonce"),
        execution_id,
        subject: "career.dynamic-golden".parse().expect("subject"),
        audience: "pitgun.verifier".parse().expect("audience"),
        initial_run_id: initial_contract.run_id().expect("initial run id"),
        initial_contract,
        decision_envelope: decision_envelope
            .artifact_identity()
            .expect("decision-envelope identity"),
        policy: artifact(
            "pitgun.racing.dynamic-instructions",
            "1.0.0",
            b"dynamic policy",
        ),
        signing_key_id: "racing-dynamic-golden-v1".parse().expect("key id"),
        validity: AuthorizationValidityV1 {
            issued_at_ms: 1_722_345_600_000,
            expires_at_ms: 1_722_345_900_000,
            late_submission_grace_ms: 900_000,
        },
    };
    let key = SigningKey::from_secret(FIXTURE_SECRET).expect("fixture signing key");
    let signed_attempt = SignedRunAttemptAuthorizationV1 {
        signature: key.sign(
            &authorization
                .signing_bytes()
                .expect("attempt signing bytes"),
        ),
        algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
        authorization,
    };
    let completed_input = RacingCompletedRunInputV1 {
        schema_version: RacingCompletedRunInputVersion::V1,
        authorized_input: signed_attempt.authorization.initial_contract.input.clone(),
        driver_instructions: RacingCompletedDriverInstructionHistoryV1 {
            instruction_profile: instruction_profile_identity.clone(),
            initial_mode: RacingDrivingMode::Balanced,
            competitor_ids: vec!["ai-01".to_string(), "player".to_string()],
            applied_timeline: RacingDriverInstructionTimelineV1 {
                schema_version: RacingDriverInstructionTimelineVersion::V1,
                events: vec![
                    instruction_event(0, "player", 1, RacingDrivingMode::Attack),
                    instruction_event(1, "ai-01", 2, RacingDrivingMode::Manage),
                    instruction_event(2, "player", 3, RacingDrivingMode::Balanced),
                ],
            },
        },
    };
    let final_contract = decision_envelope
        .final_contract_for_attempt(
            &signed_attempt.authorization,
            &completed_input,
            &instruction_profile_identity,
            &instruction_profile,
        )
        .expect("completed contract");
    let receipt = ExecutionReceiptV1::for_contract(
        &final_contract,
        execution_id,
        RuntimeIdentity {
            engine: "pitgun-rust".parse().expect("engine"),
            engine_version: "1.0.0".parse().expect("engine version"),
            target: "wasm32-unknown-unknown".parse().expect("target"),
            artifact_digest: Digest::from_bytes(b"dynamic golden wasm module"),
        },
        Digest::from_bytes(b"dynamic Racing output"),
        Digest::from_bytes(b"dynamic Racing telemetry summary"),
    )
    .expect("receipt");

    RacingDynamicAttemptEvidenceFixtureV1 {
        fixture_version: "pitgun.racing-dynamic-attempt-evidence/v1".to_string(),
        instruction_profile_identity,
        instruction_profile,
        decision_envelope,
        signed_attempt,
        completed_input,
        final_contract,
        receipt,
    }
}

fn published_fixture() -> RacingDynamicAttemptEvidenceFixtureV1 {
    serde_json::from_str(FIXTURE_JSON).expect("published dynamic-attempt fixture")
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn published_dynamic_attempt_fixture_verifies_end_to_end() {
    let fixture = published_fixture();
    assert_eq!(fixture, generated_fixture());

    let audience = "pitgun.verifier".parse().expect("audience");
    let mut keyring = VerificationKeyring::new();
    keyring.insert(
        "racing-dynamic-golden-v1".parse().expect("key id"),
        SigningKey::from_secret(FIXTURE_SECRET).expect("fixture key"),
    );
    keyring
        .verify_attempt_execution(&fixture.signed_attempt, &audience, 1_722_345_700_000)
        .expect("signed execution authorization");
    fixture
        .decision_envelope
        .validate_attempt_authorization(
            &fixture.signed_attempt.authorization,
            &fixture.instruction_profile_identity,
            &fixture.instruction_profile,
        )
        .expect("generic-to-Racing envelope binding");
    let final_contract = fixture
        .decision_envelope
        .final_contract_for_attempt(
            &fixture.signed_attempt.authorization,
            &fixture.completed_input,
            &fixture.instruction_profile_identity,
            &fixture.instruction_profile,
        )
        .expect("completed dynamic contract");
    assert_eq!(final_contract, fixture.final_contract);
    fixture
        .signed_attempt
        .authorization
        .validate_completed_receipt(&final_contract, &fixture.receipt)
        .expect("receipt bound to the final run and original execution");
    keyring
        .verify_attempt_submission(&fixture.signed_attempt, &audience, 1_722_346_800_000)
        .expect("late submission inside grace window");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn dynamic_attempt_mutations_fail_closed() {
    let fixture = published_fixture();

    let mut mutated_envelope = fixture.decision_envelope.clone();
    mutated_envelope.allowed_modes.remove(0);
    assert!(matches!(
        mutated_envelope.validate_attempt_authorization(
            &fixture.signed_attempt.authorization,
            &fixture.instruction_profile_identity,
            &fixture.instruction_profile,
        ),
        Err(RacingInstructionAuthorizationError::DecisionEnvelopeIdentityMismatch)
    ));

    let mut changed_history = fixture.completed_input.clone();
    changed_history.driver_instructions.applied_timeline.events[0].mode = RacingDrivingMode::Manage;
    let changed_contract = fixture
        .decision_envelope
        .final_contract_for_attempt(
            &fixture.signed_attempt.authorization,
            &changed_history,
            &fixture.instruction_profile_identity,
            &fixture.instruction_profile,
        )
        .expect("different but authorized history");
    assert_ne!(changed_contract.run_id().unwrap(), fixture.receipt.run_id);
    assert!(
        fixture
            .signed_attempt
            .authorization
            .validate_completed_receipt(&changed_contract, &fixture.receipt)
            .is_err()
    );

    let mut reordered_history = fixture.completed_input.clone();
    reordered_history
        .driver_instructions
        .applied_timeline
        .events
        .swap(0, 1);
    assert!(matches!(
        fixture.decision_envelope.final_contract_for_attempt(
            &fixture.signed_attempt.authorization,
            &reordered_history,
            &fixture.instruction_profile_identity,
            &fixture.instruction_profile,
        ),
        Err(RacingInstructionAuthorizationError::CompletedInput(_))
            | Err(RacingInstructionAuthorizationError::Profile(
                RacingDriverContractError::NonCanonicalInstructionOrder { .. }
            ))
    ));

    let mut bad_signature = fixture.signed_attempt;
    bad_signature.signature.replace_range(0..1, "1");
    let mut keyring = VerificationKeyring::new();
    keyring.insert(
        "racing-dynamic-golden-v1".parse().expect("key id"),
        SigningKey::from_secret(FIXTURE_SECRET).expect("fixture key"),
    );
    assert!(matches!(
        keyring.verify_attempt_execution(
            &bad_signature,
            &"pitgun.verifier".parse().expect("audience"),
            1_722_345_700_000,
        ),
        Err(AuthorizationVerificationError::InvalidSignature)
    ));
}

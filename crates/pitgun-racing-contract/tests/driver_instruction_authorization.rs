use pitgun_contract::{
    ArtifactIdentity, ContractVersion, DeterministicRunContractV1, Digest, EventOrderingV1,
    Identifier, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
    RandomAlgorithm, RandomContractV1, RuntimeProfile, ScenarioIdentity, Seed, SemanticVersion,
    StreamDerivation,
};
use pitgun_racing_contract::{
    RacingCompletedDriverInstructionHistoryV1, RacingCompletedRunError, RacingCompletedRunInputV1,
    RacingCompletedRunInputVersion, RacingDriverContractError,
    RacingDriverInstructionAuthorizationV1, RacingDriverInstructionAuthorizationVersion,
    RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionBoundaryV1,
    RacingDriverInstructionEventV1, RacingDriverInstructionProfileV1,
    RacingDriverInstructionProfileVersion, RacingDriverInstructionTimelineV1,
    RacingDriverInstructionTimelineVersion, RacingDrivingMode, RacingInstructionAuthorizationError,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

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
            id: Identifier::new("racing.weekend").expect("scenario id"),
            version: SemanticVersion::new("1.0.0").expect("scenario version"),
        },
        model: artifact("pitgun.racing-model-v3", "0.13.0", b"model"),
        data_pack: artifact("pitgun.racing", "1.6.0", b"catalog"),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(42),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 50_000, 1).expect("logical clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: Digest::from_bytes(b"authorized input"),
        },
    }
}

fn profile_identity() -> ArtifactIdentity {
    artifact(
        "pitgun.racing-driver-instructions",
        "1.0.0",
        b"instruction profile",
    )
}

fn profile() -> RacingDriverInstructionProfileV1 {
    RacingDriverInstructionProfileV1 {
        schema_version: RacingDriverInstructionProfileVersion::V1,
        default_mode: RacingDrivingMode::Balanced,
        boundary_granularity: RacingDriverInstructionBoundaryGranularityV1::LapStart,
        max_events_per_session: 8,
    }
}

fn event(
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

fn authorization() -> RacingDriverInstructionAuthorizationV1 {
    RacingDriverInstructionAuthorizationV1 {
        schema_version: RacingDriverInstructionAuthorizationVersion::V1,
        authorized_input: initial_contract().input,
        instruction_profile: profile_identity(),
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
    }
}

fn completed_input() -> RacingCompletedRunInputV1 {
    RacingCompletedRunInputV1 {
        schema_version: RacingCompletedRunInputVersion::V1,
        authorized_input: initial_contract().input,
        driver_instructions: RacingCompletedDriverInstructionHistoryV1 {
            instruction_profile: profile_identity(),
            initial_mode: RacingDrivingMode::Balanced,
            competitor_ids: vec!["ai-01".to_string(), "player".to_string()],
            applied_timeline: RacingDriverInstructionTimelineV1 {
                schema_version: RacingDriverInstructionTimelineVersion::V1,
                events: vec![
                    event(0, "player", 1, RacingDrivingMode::Attack),
                    event(1, "player", 3, RacingDrivingMode::Balanced),
                ],
            },
        },
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn bounded_authorization_accepts_exact_completed_history() {
    let authorization = authorization();
    authorization
        .validate(&initial_contract(), &profile_identity(), &profile())
        .expect("valid authorization envelope");
    authorization
        .validate_completed_input(
            &completed_input(),
            &initial_contract(),
            &profile_identity(),
            &profile(),
        )
        .expect("authorized completed history");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn narrowed_modes_and_event_limit_reject_completed_history() {
    let mut modes = authorization();
    modes.allowed_modes = vec![RacingDrivingMode::Balanced];
    assert!(matches!(
        modes.validate_completed_input(
            &completed_input(),
            &initial_contract(),
            &profile_identity(),
            &profile(),
        ),
        Err(
            RacingInstructionAuthorizationError::DisallowedInstructionMode {
                sequence: 0,
                mode: RacingDrivingMode::Attack,
            }
        )
    ));

    let mut count = authorization();
    count.max_events_per_session = 1;
    assert!(matches!(
        count.validate_completed_input(
            &completed_input(),
            &initial_contract(),
            &profile_identity(),
            &profile(),
        ),
        Err(RacingInstructionAuthorizationError::CompletedInput(
            RacingCompletedRunError::DriverContract(
                RacingDriverContractError::TooManyInstructionEvents {
                    maximum: 1,
                    actual: 2
                }
            )
        ))
    ));

    let mut competitor_scope = completed_input();
    competitor_scope
        .driver_instructions
        .competitor_ids
        .insert(1, "ai-02".to_string());
    assert!(matches!(
        authorization().validate_completed_input(
            &competitor_scope,
            &initial_contract(),
            &profile_identity(),
            &profile(),
        ),
        Err(RacingInstructionAuthorizationError::CompletedCompetitorSetMismatch)
    ));

    let mut outside_session = completed_input();
    outside_session.driver_instructions.applied_timeline.events[1]
        .effective_at
        .lap_index = 5;
    assert!(matches!(
        authorization().validate_completed_input(
            &outside_session,
            &initial_contract(),
            &profile_identity(),
            &profile(),
        ),
        Err(RacingInstructionAuthorizationError::CompletedInput(
            RacingCompletedRunError::DriverContract(
                RacingDriverContractError::InstructionBoundaryOutOfRange { sequence: 1 }
            )
        ))
    ));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn envelope_rejects_noncanonical_or_profile_widening_constraints() {
    let initial = initial_contract();
    let identity = profile_identity();
    let profile = profile();

    let mut duplicate_mode = authorization();
    duplicate_mode.allowed_modes = vec![RacingDrivingMode::Balanced, RacingDrivingMode::Balanced];
    assert!(matches!(
        duplicate_mode.validate(&initial, &identity, &profile),
        Err(RacingInstructionAuthorizationError::NonCanonicalAllowedModes { .. })
    ));

    let mut missing_default = authorization();
    missing_default.allowed_modes = vec![RacingDrivingMode::Attack];
    assert!(matches!(
        missing_default.validate(&initial, &identity, &profile),
        Err(RacingInstructionAuthorizationError::DefaultModeNotAllowed { .. })
    ));

    let mut excessive_count = authorization();
    excessive_count.max_events_per_session = profile.max_events_per_session + 1;
    assert!(matches!(
        excessive_count.validate(&initial, &identity, &profile),
        Err(RacingInstructionAuthorizationError::InvalidEventLimit)
    ));

    let mut unsorted_competitors = authorization();
    unsorted_competitors.competitor_ids.swap(0, 1);
    assert!(matches!(
        unsorted_competitors.validate(&initial, &identity, &profile),
        Err(RacingInstructionAuthorizationError::NonCanonicalCompetitorOrder { .. })
    ));

    let mut wrong_input = authorization();
    wrong_input.authorized_input.digest = Digest::from_bytes(b"other input");
    assert!(matches!(
        wrong_input.validate(&initial, &identity, &profile),
        Err(RacingInstructionAuthorizationError::AuthorizedInputMismatch)
    ));

    let mut wrong_profile = authorization();
    wrong_profile.instruction_profile = artifact(
        "pitgun.racing-driver-instructions",
        "1.0.1",
        b"other profile",
    );
    assert!(matches!(
        wrong_profile.validate(&initial, &identity, &profile),
        Err(RacingInstructionAuthorizationError::InstructionProfileIdentityMismatch)
    ));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn envelope_wire_format_is_strict_and_versioned() {
    let authorization = authorization();
    let mut encoded = serde_json::to_value(authorization).expect("authorization JSON");
    assert_eq!(
        encoded["schema_version"],
        "pitgun.racing-driver-instruction-authorization/v1"
    );
    encoded["wall_clock_timeout_ms"] = serde_json::json!(30_000);
    assert!(serde_json::from_value::<RacingDriverInstructionAuthorizationV1>(encoded).is_err());
}

use pitgun_contract::{
    ArtifactIdentity, ContractVersion, DeterministicRunContractV1, Digest, EventOrderingV1,
    Identifier, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
    RandomAlgorithm, RandomContractV1, RuntimeProfile, ScenarioIdentity, Seed, SemanticVersion,
    StreamDerivation,
};
use pitgun_racing_contract::{
    RacingCompletedDriverInstructionHistoryV1, RacingCompletedRunError, RacingCompletedRunInputV1,
    RacingCompletedRunInputVersion, RacingDriverContractError,
    RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionBoundaryV1,
    RacingDriverInstructionEventV1, RacingDriverInstructionProfileV1,
    RacingDriverInstructionProfileVersion, RacingDriverInstructionTimelineV1,
    RacingDriverInstructionTimelineVersion, RacingDrivingMode,
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

fn instruction_profile() -> RacingDriverInstructionProfileV1 {
    RacingDriverInstructionProfileV1 {
        schema_version: RacingDriverInstructionProfileVersion::V1,
        default_mode: RacingDrivingMode::Balanced,
        boundary_granularity: RacingDriverInstructionBoundaryGranularityV1::LapStart,
        max_events_per_session: 8,
    }
}

fn instruction_profile_identity() -> ArtifactIdentity {
    artifact(
        "pitgun.racing-driver-instructions",
        "1.0.0",
        b"instruction profile",
    )
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

fn completed_input() -> RacingCompletedRunInputV1 {
    RacingCompletedRunInputV1 {
        schema_version: RacingCompletedRunInputVersion::V1,
        authorized_input: initial_contract().input,
        driver_instructions: RacingCompletedDriverInstructionHistoryV1 {
            instruction_profile: instruction_profile_identity(),
            initial_mode: RacingDrivingMode::Balanced,
            competitor_ids: vec!["ai-01".to_string(), "player".to_string()],
            applied_timeline: RacingDriverInstructionTimelineV1 {
                schema_version: RacingDriverInstructionTimelineVersion::V1,
                events: vec![
                    event(0, "ai-01", 1, RacingDrivingMode::Manage),
                    event(1, "player", 1, RacingDrivingMode::Attack),
                    event(2, "player", 3, RacingDrivingMode::Balanced),
                ],
            },
        },
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn completed_instruction_history_has_a_portable_final_run_identity() {
    let initial = initial_contract();
    let identity = instruction_profile_identity();
    let profile = instruction_profile();
    let baseline = completed_input();
    let baseline_run_id = baseline
        .final_run_id(&initial, &identity, &profile, 5, 20)
        .expect("baseline final run id");

    let mut changed = baseline.clone();
    changed.driver_instructions.applied_timeline.events[1].mode = RacingDrivingMode::Manage;
    assert_ne!(
        changed
            .final_run_id(&initial, &identity, &profile, 5, 20)
            .expect("changed final run id"),
        baseline_run_id
    );
    assert_ne!(baseline_run_id, initial.run_id().expect("initial run id"));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn completed_instruction_history_fails_closed_on_noncanonical_order() {
    let mut completed = completed_input();
    completed
        .driver_instructions
        .applied_timeline
        .events
        .swap(0, 1);
    completed.driver_instructions.applied_timeline.events[0].sequence = 0;
    completed.driver_instructions.applied_timeline.events[1].sequence = 1;

    assert!(matches!(
        completed.final_run_id(
            &initial_contract(),
            &instruction_profile_identity(),
            &instruction_profile(),
            5,
            20,
        ),
        Err(RacingCompletedRunError::DriverContract(
            RacingDriverContractError::NonCanonicalInstructionOrder { .. }
        ))
    ));
}

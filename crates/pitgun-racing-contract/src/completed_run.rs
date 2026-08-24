//! Racing-specific identity for a completed run with driver instructions.

use std::{collections::BTreeSet, fmt};

use pitgun_contract::{
    ArtifactIdentity, CanonicalJsonError, DeterministicRunContractV1, Digest,
    InputCanonicalization, InputIdentity, InputMediaType, canonical_json_digest,
};
use serde::{Deserialize, Serialize};

use crate::{
    RacingDriverContractError, RacingDriverInstructionProfileV1, RacingDriverInstructionTimelineV1,
    RacingDrivingMode,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingCompletedRunInputVersion {
    #[serde(rename = "pitgun.racing-completed-run-input/v1")]
    V1,
}

/// Exact driver-instruction facts applied during one completed Racing session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingCompletedDriverInstructionHistoryV1 {
    /// Content-addressed instruction-profile resource used by the simulator.
    pub instruction_profile: ArtifactIdentity,
    /// Mode resolved at the initial session boundary for every competitor.
    pub initial_mode: RacingDrivingMode,
    /// Canonically sorted complete set of competitors governed by the profile.
    pub competitor_ids: Vec<String>,
    /// Canonically ordered transitions that affected physical execution.
    pub applied_timeline: RacingDriverInstructionTimelineV1,
}

/// Canonical completed input layered over the input authorized before execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingCompletedRunInputV1 {
    pub schema_version: RacingCompletedRunInputVersion,
    /// Preserves the exact input identity covered by the initial authorization.
    pub authorized_input: InputIdentity,
    pub driver_instructions: RacingCompletedDriverInstructionHistoryV1,
}

#[derive(Debug)]
pub enum RacingCompletedRunError {
    AuthorizedInputMismatch,
    InstructionProfileIdentityMismatch,
    EmptyCompetitorSet,
    InvalidCompetitorId {
        index: usize,
    },
    NonCanonicalCompetitorOrder {
        index: usize,
    },
    InitialModeMismatch {
        expected: RacingDrivingMode,
        actual: RacingDrivingMode,
    },
    DriverContract(RacingDriverContractError),
    CanonicalJson(CanonicalJsonError),
}

impl fmt::Display for RacingCompletedRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizedInputMismatch => formatter.write_str(
                "completed Racing input does not reference the initially authorized input",
            ),
            Self::InstructionProfileIdentityMismatch => formatter.write_str(
                "completed Racing input does not reference the resolved instruction profile",
            ),
            Self::EmptyCompetitorSet => {
                formatter.write_str("completed Racing input must identify its competitors")
            }
            Self::InvalidCompetitorId { index } => write!(
                formatter,
                "completed Racing input has an empty or non-canonical competitor id at index {index}",
            ),
            Self::NonCanonicalCompetitorOrder { index } => write!(
                formatter,
                "completed Racing competitor ids are not strictly sorted at index {index}",
            ),
            Self::InitialModeMismatch { expected, actual } => write!(
                formatter,
                "completed Racing initial mode {actual:?} does not match profile default {expected:?}",
            ),
            Self::DriverContract(error) => error.fmt(formatter),
            Self::CanonicalJson(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RacingCompletedRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DriverContract(error) => Some(error),
            Self::CanonicalJson(error) => Some(error),
            Self::AuthorizedInputMismatch
            | Self::InstructionProfileIdentityMismatch
            | Self::EmptyCompetitorSet
            | Self::InvalidCompetitorId { .. }
            | Self::NonCanonicalCompetitorOrder { .. }
            | Self::InitialModeMismatch { .. } => None,
        }
    }
}

impl From<RacingDriverContractError> for RacingCompletedRunError {
    fn from(error: RacingDriverContractError) -> Self {
        Self::DriverContract(error)
    }
}

impl From<CanonicalJsonError> for RacingCompletedRunError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

impl RacingCompletedRunInputV1 {
    /// Validates completed Racing facts against the exact resolved session.
    pub fn validate(
        &self,
        initial_contract: &DeterministicRunContractV1,
        instruction_profile_identity: &ArtifactIdentity,
        instruction_profile: &RacingDriverInstructionProfileV1,
        lap_count: u16,
        segment_count: u32,
    ) -> Result<(), RacingCompletedRunError> {
        if self.authorized_input != initial_contract.input {
            return Err(RacingCompletedRunError::AuthorizedInputMismatch);
        }
        if self.driver_instructions.instruction_profile != *instruction_profile_identity {
            return Err(RacingCompletedRunError::InstructionProfileIdentityMismatch);
        }
        if self.driver_instructions.initial_mode != instruction_profile.default_mode {
            return Err(RacingCompletedRunError::InitialModeMismatch {
                expected: instruction_profile.default_mode,
                actual: self.driver_instructions.initial_mode,
            });
        }

        let competitor_ids = canonical_competitor_set(&self.driver_instructions.competitor_ids)?;
        self.driver_instructions
            .applied_timeline
            .validate_for_session(
                instruction_profile,
                &competitor_ids,
                lap_count,
                segment_count,
            )?;
        Ok(())
    }

    /// Derives the final run contract without mutating the initial authorization.
    ///
    /// The resulting input digest commits to both the authorized input identity and
    /// every driver instruction that actually affected the simulation.
    pub fn final_contract(
        &self,
        initial_contract: &DeterministicRunContractV1,
        instruction_profile_identity: &ArtifactIdentity,
        instruction_profile: &RacingDriverInstructionProfileV1,
        lap_count: u16,
        segment_count: u32,
    ) -> Result<DeterministicRunContractV1, RacingCompletedRunError> {
        self.validate(
            initial_contract,
            instruction_profile_identity,
            instruction_profile,
            lap_count,
            segment_count,
        )?;

        let mut final_contract = initial_contract.clone();
        final_contract.input = InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: canonical_json_digest(self)?,
        };
        Ok(final_contract)
    }

    /// Calculates the completed logical run identity.
    pub fn final_run_id(
        &self,
        initial_contract: &DeterministicRunContractV1,
        instruction_profile_identity: &ArtifactIdentity,
        instruction_profile: &RacingDriverInstructionProfileV1,
        lap_count: u16,
        segment_count: u32,
    ) -> Result<Digest, RacingCompletedRunError> {
        Ok(self
            .final_contract(
                initial_contract,
                instruction_profile_identity,
                instruction_profile,
                lap_count,
                segment_count,
            )?
            .run_id()?)
    }
}

fn canonical_competitor_set(
    competitor_ids: &[String],
) -> Result<BTreeSet<String>, RacingCompletedRunError> {
    if competitor_ids.is_empty() {
        return Err(RacingCompletedRunError::EmptyCompetitorSet);
    }

    for (index, competitor_id) in competitor_ids.iter().enumerate() {
        if competitor_id.is_empty() || competitor_id.trim() != competitor_id {
            return Err(RacingCompletedRunError::InvalidCompetitorId { index });
        }
        if index > 0 && competitor_ids[index - 1] >= *competitor_id {
            return Err(RacingCompletedRunError::NonCanonicalCompetitorOrder { index });
        }
    }

    Ok(competitor_ids.iter().cloned().collect())
}

#[cfg(test)]
mod tests {
    use pitgun_contract::{
        ContractVersion, EventOrderingV1, Identifier, LogicalClockV1, RandomAlgorithm,
        RandomContractV1, RuntimeProfile, ScenarioIdentity, Seed, SemanticVersion,
        StreamDerivation,
    };

    use super::*;
    use crate::{
        RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionBoundaryV1,
        RacingDriverInstructionEventV1, RacingDriverInstructionProfileVersion,
        RacingDriverInstructionTimelineVersion,
    };

    fn artifact(id: &str, version: &str, contents: &[u8]) -> ArtifactIdentity {
        ArtifactIdentity {
            id: Identifier::new(id).expect("artifact id"),
            version: SemanticVersion::new(version).expect("artifact version"),
            digest: Digest::from_bytes(contents),
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
        let initial = initial_contract();
        RacingCompletedRunInputV1 {
            schema_version: RacingCompletedRunInputVersion::V1,
            authorized_input: initial.input,
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

    #[test]
    fn completed_history_changes_final_run_identity_without_mutating_authorization() {
        let initial = initial_contract();
        let profile = instruction_profile();
        let profile_identity = instruction_profile_identity();
        let baseline = completed_input();
        let baseline_run_id = baseline
            .final_run_id(&initial, &profile_identity, &profile, 5, 20)
            .expect("baseline run id");

        let mut changed_mode = baseline.clone();
        changed_mode.driver_instructions.applied_timeline.events[1].mode =
            RacingDrivingMode::Manage;
        assert_ne!(
            changed_mode
                .final_run_id(&initial, &profile_identity, &profile, 5, 20)
                .expect("changed-mode run id"),
            baseline_run_id
        );

        let mut missing_event = baseline.clone();
        missing_event
            .driver_instructions
            .applied_timeline
            .events
            .pop();
        assert_ne!(
            missing_event
                .final_run_id(&initial, &profile_identity, &profile, 5, 20)
                .expect("missing-event run id"),
            baseline_run_id
        );
        assert_eq!(initial, initial_contract());
    }

    #[test]
    fn completed_history_rejects_noncanonical_or_unknown_events() {
        let initial = initial_contract();
        let profile = instruction_profile();
        let profile_identity = instruction_profile_identity();

        let mut reordered = completed_input();
        reordered
            .driver_instructions
            .applied_timeline
            .events
            .swap(0, 1);
        reordered.driver_instructions.applied_timeline.events[0].sequence = 0;
        reordered.driver_instructions.applied_timeline.events[1].sequence = 1;
        assert!(matches!(
            reordered.final_run_id(&initial, &profile_identity, &profile, 5, 20),
            Err(RacingCompletedRunError::DriverContract(
                RacingDriverContractError::NonCanonicalInstructionOrder { .. }
            ))
        ));

        let mut unknown = completed_input();
        unknown.driver_instructions.applied_timeline.events[0].competitor_id = "ghost".to_string();
        assert!(matches!(
            unknown.final_run_id(&initial, &profile_identity, &profile, 5, 20),
            Err(RacingCompletedRunError::DriverContract(
                RacingDriverContractError::UnknownInstructionCompetitor { .. }
            ))
        ));
    }

    #[test]
    fn completed_history_requires_canonical_competitors_and_profile_default() {
        let initial = initial_contract();
        let profile = instruction_profile();
        let profile_identity = instruction_profile_identity();

        let mut unsorted = completed_input();
        unsorted.driver_instructions.competitor_ids.swap(0, 1);
        assert!(matches!(
            unsorted.final_run_id(&initial, &profile_identity, &profile, 5, 20),
            Err(RacingCompletedRunError::NonCanonicalCompetitorOrder { .. })
        ));

        let mut wrong_initial_mode = completed_input();
        wrong_initial_mode.driver_instructions.initial_mode = RacingDrivingMode::Attack;
        assert!(matches!(
            wrong_initial_mode.final_run_id(&initial, &profile_identity, &profile, 5, 20),
            Err(RacingCompletedRunError::InitialModeMismatch { .. })
        ));

        let wrong_profile_identity = artifact(
            "pitgun.racing-driver-instructions",
            "1.0.1",
            b"different instruction profile",
        );
        assert!(matches!(
            completed_input().final_run_id(&initial, &wrong_profile_identity, &profile, 5, 20,),
            Err(RacingCompletedRunError::InstructionProfileIdentityMismatch)
        ));
    }

    #[test]
    fn empty_completed_history_is_valid_and_deterministic() {
        let initial = initial_contract();
        let profile = instruction_profile();
        let profile_identity = instruction_profile_identity();
        let mut empty = completed_input();
        empty.driver_instructions.applied_timeline.events.clear();

        let first = empty
            .final_run_id(&initial, &profile_identity, &profile, 5, 20)
            .expect("first empty-history run id");
        let second = empty
            .final_run_id(&initial, &profile_identity, &profile, 5, 20)
            .expect("second empty-history run id");
        assert_eq!(first, second);
        assert_ne!(first, initial.run_id().expect("initial run id"));
    }

    #[test]
    fn completed_history_fails_closed_on_wire_extensions() {
        let input = completed_input();
        let mut encoded = serde_json::to_value(input).expect("completed input JSON");
        encoded["driver_instructions"]["event_source"] = serde_json::json!("browser");
        assert!(serde_json::from_value::<RacingCompletedRunInputV1>(encoded).is_err());
    }
}

//! Bounded Racing authorization for driver instructions not known at issuance.

use std::fmt;

use pitgun_contract::{ArtifactIdentity, DeterministicRunContractV1, InputIdentity};
use serde::{Deserialize, Serialize};

use crate::{
    RacingCompletedRunError, RacingCompletedRunInputV1, RacingDriverContractError,
    RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionProfileV1,
    RacingDrivingMode,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingDriverInstructionAuthorizationVersion {
    #[serde(rename = "pitgun.racing-driver-instruction-authorization/v1")]
    V1,
}

/// Authority-owned limits for decisions that may be applied during one session.
///
/// The envelope contains no future event. It narrows the immutable catalog
/// profile and can therefore be signed before live player or AI decisions exist.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverInstructionAuthorizationV1 {
    pub schema_version: RacingDriverInstructionAuthorizationVersion,
    pub authorized_input: InputIdentity,
    pub instruction_profile: ArtifactIdentity,
    pub allowed_modes: Vec<RacingDrivingMode>,
    pub boundary_granularity: RacingDriverInstructionBoundaryGranularityV1,
    pub max_events_per_session: u16,
    pub competitor_ids: Vec<String>,
    pub lap_count: u16,
    pub segment_count: u32,
}

#[derive(Debug)]
pub enum RacingInstructionAuthorizationError {
    AuthorizedInputMismatch,
    InstructionProfileIdentityMismatch,
    EmptyAllowedModes,
    NonCanonicalAllowedModes {
        index: usize,
    },
    DefaultModeNotAllowed {
        default_mode: RacingDrivingMode,
    },
    BoundaryGranularityMismatch,
    InvalidEventLimit,
    EmptyCompetitorSet,
    InvalidCompetitorId {
        index: usize,
    },
    NonCanonicalCompetitorOrder {
        index: usize,
    },
    InvalidSessionDimensions,
    CompletedCompetitorSetMismatch,
    DisallowedInstructionMode {
        sequence: u32,
        mode: RacingDrivingMode,
    },
    Profile(RacingDriverContractError),
    CompletedInput(RacingCompletedRunError),
}

impl fmt::Display for RacingInstructionAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizedInputMismatch => formatter.write_str(
                "Racing instruction authorization does not bind the initial input",
            ),
            Self::InstructionProfileIdentityMismatch => formatter.write_str(
                "Racing instruction authorization does not bind the resolved profile",
            ),
            Self::EmptyAllowedModes => {
                formatter.write_str("Racing instruction authorization must allow a mode")
            }
            Self::NonCanonicalAllowedModes { index } => write!(
                formatter,
                "Racing instruction modes are not strictly ordered at index {index}",
            ),
            Self::DefaultModeNotAllowed { default_mode } => write!(
                formatter,
                "Racing instruction authorization excludes profile default {default_mode:?}",
            ),
            Self::BoundaryGranularityMismatch => formatter.write_str(
                "Racing instruction authorization changes the profile boundary granularity",
            ),
            Self::InvalidEventLimit => formatter.write_str(
                "Racing instruction authorization event limit must be positive and no greater than the profile limit",
            ),
            Self::EmptyCompetitorSet => formatter.write_str(
                "Racing instruction authorization must identify its competitors",
            ),
            Self::InvalidCompetitorId { index } => write!(
                formatter,
                "Racing instruction authorization has an empty or non-canonical competitor id at index {index}",
            ),
            Self::NonCanonicalCompetitorOrder { index } => write!(
                formatter,
                "Racing instruction authorization competitor ids are not strictly sorted at index {index}",
            ),
            Self::InvalidSessionDimensions => formatter.write_str(
                "Racing instruction authorization requires positive lap and segment counts",
            ),
            Self::CompletedCompetitorSetMismatch => formatter.write_str(
                "completed Racing input does not use the authorized competitor set",
            ),
            Self::DisallowedInstructionMode { sequence, mode } => write!(
                formatter,
                "completed Racing instruction {sequence} uses unauthorized mode {mode:?}",
            ),
            Self::Profile(error) => error.fmt(formatter),
            Self::CompletedInput(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RacingInstructionAuthorizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Profile(error) => Some(error),
            Self::CompletedInput(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RacingCompletedRunError> for RacingInstructionAuthorizationError {
    fn from(error: RacingCompletedRunError) -> Self {
        Self::CompletedInput(error)
    }
}

impl RacingDriverInstructionAuthorizationV1 {
    /// Validates that this authorization only narrows the resolved profile and session.
    pub fn validate(
        &self,
        initial_contract: &DeterministicRunContractV1,
        instruction_profile_identity: &ArtifactIdentity,
        instruction_profile: &RacingDriverInstructionProfileV1,
    ) -> Result<(), RacingInstructionAuthorizationError> {
        instruction_profile
            .validate()
            .map_err(RacingInstructionAuthorizationError::Profile)?;
        if self.authorized_input != initial_contract.input {
            return Err(RacingInstructionAuthorizationError::AuthorizedInputMismatch);
        }
        if self.instruction_profile != *instruction_profile_identity {
            return Err(RacingInstructionAuthorizationError::InstructionProfileIdentityMismatch);
        }
        validate_modes(&self.allowed_modes)?;
        if !self
            .allowed_modes
            .contains(&instruction_profile.default_mode)
        {
            return Err(RacingInstructionAuthorizationError::DefaultModeNotAllowed {
                default_mode: instruction_profile.default_mode,
            });
        }
        if self.boundary_granularity != instruction_profile.boundary_granularity {
            return Err(RacingInstructionAuthorizationError::BoundaryGranularityMismatch);
        }
        if self.max_events_per_session == 0
            || self.max_events_per_session > instruction_profile.max_events_per_session
        {
            return Err(RacingInstructionAuthorizationError::InvalidEventLimit);
        }
        validate_competitors(&self.competitor_ids)?;
        if self.lap_count == 0 || self.segment_count == 0 {
            return Err(RacingInstructionAuthorizationError::InvalidSessionDimensions);
        }
        Ok(())
    }

    /// Verifies that completed physical execution stayed inside this envelope.
    pub fn validate_completed_input(
        &self,
        completed: &RacingCompletedRunInputV1,
        initial_contract: &DeterministicRunContractV1,
        instruction_profile_identity: &ArtifactIdentity,
        instruction_profile: &RacingDriverInstructionProfileV1,
    ) -> Result<(), RacingInstructionAuthorizationError> {
        self.validate(
            initial_contract,
            instruction_profile_identity,
            instruction_profile,
        )?;

        let narrowed_profile = RacingDriverInstructionProfileV1 {
            schema_version: instruction_profile.schema_version,
            default_mode: instruction_profile.default_mode,
            boundary_granularity: self.boundary_granularity,
            max_events_per_session: self.max_events_per_session,
        };
        completed.validate(
            initial_contract,
            instruction_profile_identity,
            &narrowed_profile,
            self.lap_count,
            self.segment_count,
        )?;

        if completed.driver_instructions.competitor_ids != self.competitor_ids {
            return Err(RacingInstructionAuthorizationError::CompletedCompetitorSetMismatch);
        }
        for event in &completed.driver_instructions.applied_timeline.events {
            if !self.allowed_modes.contains(&event.mode) {
                return Err(
                    RacingInstructionAuthorizationError::DisallowedInstructionMode {
                        sequence: event.sequence,
                        mode: event.mode,
                    },
                );
            }
        }
        Ok(())
    }
}

fn validate_modes(
    allowed_modes: &[RacingDrivingMode],
) -> Result<(), RacingInstructionAuthorizationError> {
    if allowed_modes.is_empty() {
        return Err(RacingInstructionAuthorizationError::EmptyAllowedModes);
    }
    for index in 1..allowed_modes.len() {
        if allowed_modes[index - 1] >= allowed_modes[index] {
            return Err(RacingInstructionAuthorizationError::NonCanonicalAllowedModes { index });
        }
    }
    Ok(())
}

fn validate_competitors(
    competitor_ids: &[String],
) -> Result<(), RacingInstructionAuthorizationError> {
    if competitor_ids.is_empty() {
        return Err(RacingInstructionAuthorizationError::EmptyCompetitorSet);
    }
    for (index, competitor_id) in competitor_ids.iter().enumerate() {
        if competitor_id.is_empty() || competitor_id.trim() != competitor_id {
            return Err(RacingInstructionAuthorizationError::InvalidCompetitorId { index });
        }
        if index > 0 && competitor_ids[index - 1] >= *competitor_id {
            return Err(RacingInstructionAuthorizationError::NonCanonicalCompetitorOrder { index });
        }
    }
    Ok(())
}

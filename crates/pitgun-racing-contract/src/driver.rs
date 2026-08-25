//! Versioned Racing driver-control contracts.

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingDriverResourceVersion {
    #[serde(rename = "pitgun.racing-driver/v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingDriverControlProfileVersion {
    #[serde(rename = "pitgun.racing-driver-control-profile/v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingDriverInstructionProfileVersion {
    #[serde(rename = "pitgun.racing-driver-instruction-profile/v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingDriverInstructionTimelineVersion {
    #[serde(rename = "pitgun.racing-driver-instruction-timeline/v1")]
    V1,
}

/// Stable catalog resource identifier for the V1 driver-instruction profile.
pub const RACING_DRIVER_INSTRUCTION_PROFILE_ID: &str = "pitgun.racing-driver-instructions";
/// Semantic version paired with the V1 driver-instruction profile schema.
pub const RACING_DRIVER_INSTRUCTION_PROFILE_VERSION: &str = "1.0.0";

/// Explicit commitment requested for one competitor and session.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RacingDrivingMode {
    Manage,
    Balanced,
    Attack,
}

/// Smallest deterministic boundary at which one profile accepts instructions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RacingDriverInstructionBoundaryGranularityV1 {
    LapStart,
    Segment,
}

/// Catalog-owned limits shared by the player and every AI competitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverInstructionProfileV1 {
    pub schema_version: RacingDriverInstructionProfileVersion,
    pub default_mode: RacingDrivingMode,
    pub boundary_granularity: RacingDriverInstructionBoundaryGranularityV1,
    pub max_events_per_session: u16,
}

/// Zero-based coordinate in one resolved session and track.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverInstructionBoundaryV1 {
    pub lap_index: u16,
    pub segment_index: u32,
}

/// One accepted transition. Event source does not alter physical semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverInstructionEventV1 {
    pub sequence: u32,
    pub competitor_id: String,
    pub effective_at: RacingDriverInstructionBoundaryV1,
    pub mode: RacingDrivingMode,
}

/// Canonically ordered transitions authored before or during one session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverInstructionTimelineV1 {
    pub schema_version: RacingDriverInstructionTimelineVersion,
    #[serde(default)]
    pub events: Vec<RacingDriverInstructionEventV1>,
}

/// Persistent physical traits owned by one versioned driver resource.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverTraitsV1 {
    pub limit_exploitation: f64,
    pub consistency: f64,
    pub tire_management: f64,
}

/// Driver resource used by the next Model V3 driver-control identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverResourceV2 {
    pub schema_version: RacingDriverResourceVersion,
    pub id: String,
    pub traits: RacingDriverTraitsV1,
}

/// Commitment targets selected by the three player-facing modes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDrivingModeCommitmentsV1 {
    pub manage: f64,
    pub balanced: f64,
    pub attack: f64,
}

/// Physical utilization response for one force channel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverUtilizationResponseV1 {
    pub floor: f64,
    pub span: f64,
}

/// Governed coefficients translating traits and mode into physical controls.
///
/// The contract fixes equation inputs, not their calibration. Candidate values
/// must be screened locally and on Databricks before a catalog can publish them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverControlProfileV1 {
    pub schema_version: RacingDriverControlProfileVersion,
    pub mode_commitments: RacingDrivingModeCommitmentsV1,
    pub cornering: RacingDriverUtilizationResponseV1,
    pub braking: RacingDriverUtilizationResponseV1,
    pub traction: RacingDriverUtilizationResponseV1,
    pub base_control_error: f64,
    pub commitment_error_gain: f64,
    pub commitment_error_exponent: f64,
    pub correction_workload_gain: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RacingDriverContractError {
    EmptyDriverId,
    InvalidTrait(&'static str),
    InvalidModeCommitments,
    InvalidUtilization(&'static str),
    InvalidControlError,
    InvalidCommitmentErrorExponent,
    InvalidCorrectionWorkloadGain,
    InvalidInstructionEventLimit,
    InvalidInstructionValidationContext,
    TooManyInstructionEvents {
        maximum: u16,
        actual: usize,
    },
    NonContiguousInstructionSequence {
        expected: u32,
        actual: u32,
    },
    EmptyInstructionCompetitor {
        sequence: u32,
    },
    UnknownInstructionCompetitor {
        sequence: u32,
        competitor_id: String,
    },
    InstructionBoundaryOutOfRange {
        sequence: u32,
    },
    InstructionAtInitialBoundary {
        sequence: u32,
    },
    UnsupportedInstructionBoundary {
        sequence: u32,
        granularity: RacingDriverInstructionBoundaryGranularityV1,
    },
    DuplicateInstructionBoundary {
        sequence: u32,
        competitor_id: String,
    },
    NonCanonicalInstructionOrder {
        sequence: u32,
    },
}

impl fmt::Display for RacingDriverContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDriverId => formatter.write_str("Racing driver id must not be empty"),
            Self::InvalidTrait(name) => {
                write!(formatter, "Racing driver trait {name} must be finite and in [0, 1]")
            }
            Self::InvalidModeCommitments => formatter.write_str(
                "Racing driving-mode commitments must be finite, ordered manage < balanced < attack, and in [0, 1]",
            ),
            Self::InvalidUtilization(channel) => write!(
                formatter,
                "Racing driver {channel} utilization must use a finite floor in [0.5, 1], a non-negative span, and a sum no greater than 1",
            ),
            Self::InvalidControlError => formatter.write_str(
                "Racing driver control-error coefficients must be finite, non-negative, and sum to at most 0.25",
            ),
            Self::InvalidCommitmentErrorExponent => formatter.write_str(
                "Racing driver commitment-error exponent must be finite and in [1, 4]",
            ),
            Self::InvalidCorrectionWorkloadGain => formatter.write_str(
                "Racing driver correction-workload gain must be finite and in [0, 10]",
            ),
            Self::InvalidInstructionEventLimit => formatter.write_str(
                "Racing driver-instruction event limit must be in [1, 4096]",
            ),
            Self::InvalidInstructionValidationContext => formatter.write_str(
                "Racing driver-instruction validation requires competitors, laps, and track segments",
            ),
            Self::TooManyInstructionEvents { maximum, actual } => write!(
                formatter,
                "Racing driver-instruction timeline has {actual} events but profile allows at most {maximum}",
            ),
            Self::NonContiguousInstructionSequence { expected, actual } => write!(
                formatter,
                "Racing driver-instruction sequence must be contiguous: expected {expected}, got {actual}",
            ),
            Self::EmptyInstructionCompetitor { sequence } => write!(
                formatter,
                "Racing driver-instruction event {sequence} has an empty competitor id",
            ),
            Self::UnknownInstructionCompetitor {
                sequence,
                competitor_id,
            } => write!(
                formatter,
                "Racing driver-instruction event {sequence} targets unknown competitor {competitor_id:?}",
            ),
            Self::InstructionBoundaryOutOfRange { sequence } => write!(
                formatter,
                "Racing driver-instruction event {sequence} targets a boundary outside the resolved session",
            ),
            Self::InstructionAtInitialBoundary { sequence } => write!(
                formatter,
                "Racing driver-instruction event {sequence} cannot replace the common mode at the initial boundary",
            ),
            Self::UnsupportedInstructionBoundary {
                sequence,
                granularity,
            } => write!(
                formatter,
                "Racing driver-instruction event {sequence} uses a boundary unsupported by {granularity:?}",
            ),
            Self::DuplicateInstructionBoundary {
                sequence,
                competitor_id,
            } => write!(
                formatter,
                "Racing driver-instruction event {sequence} duplicates a transition for competitor {competitor_id:?} at one boundary",
            ),
            Self::NonCanonicalInstructionOrder { sequence } => write!(
                formatter,
                "Racing driver-instruction event {sequence} is not ordered by boundary then competitor id",
            ),
        }
    }
}

impl std::error::Error for RacingDriverContractError {}

impl RacingDriverResourceV2 {
    pub fn validate(&self) -> Result<(), RacingDriverContractError> {
        if self.id.trim().is_empty() {
            return Err(RacingDriverContractError::EmptyDriverId);
        }
        validate_unit_trait("limit_exploitation", self.traits.limit_exploitation)?;
        validate_unit_trait("consistency", self.traits.consistency)?;
        validate_unit_trait("tire_management", self.traits.tire_management)
    }
}

impl RacingDriverInstructionProfileV1 {
    pub fn validate(&self) -> Result<(), RacingDriverContractError> {
        if !(1..=4096).contains(&self.max_events_per_session) {
            return Err(RacingDriverContractError::InvalidInstructionEventLimit);
        }
        Ok(())
    }
}

impl RacingDriverInstructionTimelineV1 {
    /// Validates already-canonical authoring bytes against one resolved session.
    ///
    /// Events are never silently sorted or clamped because doing so would make
    /// malformed authoring input share an execution identity with valid input.
    pub fn validate_for_session(
        &self,
        profile: &RacingDriverInstructionProfileV1,
        competitor_ids: &BTreeSet<String>,
        lap_count: u16,
        segment_count: u32,
    ) -> Result<(), RacingDriverContractError> {
        profile.validate()?;
        if competitor_ids.is_empty() || lap_count == 0 || segment_count == 0 {
            return Err(RacingDriverContractError::InvalidInstructionValidationContext);
        }
        if self.events.len() > usize::from(profile.max_events_per_session) {
            return Err(RacingDriverContractError::TooManyInstructionEvents {
                maximum: profile.max_events_per_session,
                actual: self.events.len(),
            });
        }

        let mut previous: Option<(&RacingDriverInstructionBoundaryV1, &str)> = None;
        for (expected_sequence, event) in self.events.iter().enumerate() {
            let expected_sequence = u32::try_from(expected_sequence)
                .map_err(|_| RacingDriverContractError::InvalidInstructionEventLimit)?;
            if event.sequence != expected_sequence {
                return Err(
                    RacingDriverContractError::NonContiguousInstructionSequence {
                        expected: expected_sequence,
                        actual: event.sequence,
                    },
                );
            }
            if event.competitor_id.trim().is_empty() {
                return Err(RacingDriverContractError::EmptyInstructionCompetitor {
                    sequence: event.sequence,
                });
            }
            if !competitor_ids.contains(&event.competitor_id) {
                return Err(RacingDriverContractError::UnknownInstructionCompetitor {
                    sequence: event.sequence,
                    competitor_id: event.competitor_id.clone(),
                });
            }
            if event.effective_at.lap_index >= lap_count
                || event.effective_at.segment_index >= segment_count
            {
                return Err(RacingDriverContractError::InstructionBoundaryOutOfRange {
                    sequence: event.sequence,
                });
            }
            if event.effective_at.lap_index == 0 && event.effective_at.segment_index == 0 {
                return Err(RacingDriverContractError::InstructionAtInitialBoundary {
                    sequence: event.sequence,
                });
            }
            if profile.boundary_granularity
                == RacingDriverInstructionBoundaryGranularityV1::LapStart
                && event.effective_at.segment_index != 0
            {
                return Err(RacingDriverContractError::UnsupportedInstructionBoundary {
                    sequence: event.sequence,
                    granularity: profile.boundary_granularity,
                });
            }

            if let Some((previous_boundary, previous_competitor)) = previous {
                match event.effective_at.cmp(previous_boundary) {
                    std::cmp::Ordering::Less => {
                        return Err(RacingDriverContractError::NonCanonicalInstructionOrder {
                            sequence: event.sequence,
                        });
                    }
                    std::cmp::Ordering::Equal => {
                        match event.competitor_id.as_str().cmp(previous_competitor) {
                            std::cmp::Ordering::Less => {
                                return Err(
                                    RacingDriverContractError::NonCanonicalInstructionOrder {
                                        sequence: event.sequence,
                                    },
                                );
                            }
                            std::cmp::Ordering::Equal => {
                                return Err(
                                    RacingDriverContractError::DuplicateInstructionBoundary {
                                        sequence: event.sequence,
                                        competitor_id: event.competitor_id.clone(),
                                    },
                                );
                            }
                            std::cmp::Ordering::Greater => {}
                        }
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
            previous = Some((&event.effective_at, &event.competitor_id));
        }
        Ok(())
    }
}

impl RacingDriverControlProfileV1 {
    pub fn validate(&self) -> Result<(), RacingDriverContractError> {
        let modes = self.mode_commitments;
        let commitments_are_valid = [modes.manage, modes.balanced, modes.attack]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            && modes.manage < modes.balanced
            && modes.balanced < modes.attack;
        if !commitments_are_valid {
            return Err(RacingDriverContractError::InvalidModeCommitments);
        }
        validate_utilization("cornering", self.cornering)?;
        validate_utilization("braking", self.braking)?;
        validate_utilization("traction", self.traction)?;
        if !self.base_control_error.is_finite()
            || !self.commitment_error_gain.is_finite()
            || self.base_control_error < 0.0
            || self.commitment_error_gain < 0.0
            || self.base_control_error + self.commitment_error_gain > 0.25
        {
            return Err(RacingDriverContractError::InvalidControlError);
        }
        if !self.commitment_error_exponent.is_finite()
            || !(1.0..=4.0).contains(&self.commitment_error_exponent)
        {
            return Err(RacingDriverContractError::InvalidCommitmentErrorExponent);
        }
        if !self.correction_workload_gain.is_finite()
            || !(0.0..=10.0).contains(&self.correction_workload_gain)
        {
            return Err(RacingDriverContractError::InvalidCorrectionWorkloadGain);
        }
        Ok(())
    }

    #[must_use]
    pub const fn commitment_for(&self, mode: RacingDrivingMode) -> f64 {
        match mode {
            RacingDrivingMode::Manage => self.mode_commitments.manage,
            RacingDrivingMode::Balanced => self.mode_commitments.balanced,
            RacingDrivingMode::Attack => self.mode_commitments.attack,
        }
    }
}

fn validate_unit_trait(name: &'static str, value: f64) -> Result<(), RacingDriverContractError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(RacingDriverContractError::InvalidTrait(name));
    }
    Ok(())
}

fn validate_utilization(
    channel: &'static str,
    response: RacingDriverUtilizationResponseV1,
) -> Result<(), RacingDriverContractError> {
    if !response.floor.is_finite()
        || !response.span.is_finite()
        || !(0.5..=1.0).contains(&response.floor)
        || response.span < 0.0
        || response.floor + response.span > 1.0
    {
        return Err(RacingDriverContractError::InvalidUtilization(channel));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_driver() -> RacingDriverResourceV2 {
        RacingDriverResourceV2 {
            schema_version: RacingDriverResourceVersion::V2,
            id: "smooth_operator".to_string(),
            traits: RacingDriverTraitsV1 {
                limit_exploitation: 0.82,
                consistency: 0.94,
                tire_management: 0.91,
            },
        }
    }

    fn valid_profile() -> RacingDriverControlProfileV1 {
        RacingDriverControlProfileV1 {
            schema_version: RacingDriverControlProfileVersion::V1,
            mode_commitments: RacingDrivingModeCommitmentsV1 {
                manage: 0.6,
                balanced: 0.8,
                attack: 1.0,
            },
            cornering: RacingDriverUtilizationResponseV1 {
                floor: 0.8,
                span: 0.2,
            },
            braking: RacingDriverUtilizationResponseV1 {
                floor: 0.78,
                span: 0.22,
            },
            traction: RacingDriverUtilizationResponseV1 {
                floor: 0.82,
                span: 0.18,
            },
            base_control_error: 0.005,
            commitment_error_gain: 0.08,
            commitment_error_exponent: 2.0,
            correction_workload_gain: 2.0,
        }
    }

    fn valid_instruction_profile() -> RacingDriverInstructionProfileV1 {
        RacingDriverInstructionProfileV1 {
            schema_version: RacingDriverInstructionProfileVersion::V1,
            default_mode: RacingDrivingMode::Balanced,
            boundary_granularity: RacingDriverInstructionBoundaryGranularityV1::LapStart,
            max_events_per_session: 8,
        }
    }

    fn competitors() -> BTreeSet<String> {
        BTreeSet::from(["ai-01".to_string(), "player".to_string()])
    }

    fn instruction(
        sequence: u32,
        competitor_id: &str,
        lap_index: u16,
        segment_index: u32,
        mode: RacingDrivingMode,
    ) -> RacingDriverInstructionEventV1 {
        RacingDriverInstructionEventV1 {
            sequence,
            competitor_id: competitor_id.to_string(),
            effective_at: RacingDriverInstructionBoundaryV1 {
                lap_index,
                segment_index,
            },
            mode,
        }
    }

    fn valid_timeline() -> RacingDriverInstructionTimelineV1 {
        RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: vec![
                instruction(0, "ai-01", 1, 0, RacingDrivingMode::Manage),
                instruction(1, "player", 1, 0, RacingDrivingMode::Attack),
                instruction(2, "player", 3, 0, RacingDrivingMode::Balanced),
            ],
        }
    }

    #[test]
    fn validates_driver_traits_without_silent_clamping() {
        assert!(valid_driver().validate().is_ok());

        let mut invalid = valid_driver();
        invalid.traits.consistency = 1.01;
        assert_eq!(
            invalid.validate(),
            Err(RacingDriverContractError::InvalidTrait("consistency"))
        );
    }

    #[test]
    fn requires_strictly_ordered_mode_commitments() {
        let mut profile = valid_profile();
        profile.mode_commitments.attack = profile.mode_commitments.balanced;
        assert_eq!(
            profile.validate(),
            Err(RacingDriverContractError::InvalidModeCommitments)
        );
    }

    #[test]
    fn exposes_named_mode_commitments_without_policy_logic() {
        let profile = valid_profile();
        profile.validate().expect("valid driver-control profile");
        assert_eq!(profile.commitment_for(RacingDrivingMode::Manage), 0.6);
        assert_eq!(profile.commitment_for(RacingDrivingMode::Balanced), 0.8);
        assert_eq!(profile.commitment_for(RacingDrivingMode::Attack), 1.0);
    }

    #[test]
    fn wire_format_uses_versioned_resources_and_snake_case_modes() {
        let driver = valid_driver();
        let encoded = serde_json::to_value(&driver).expect("driver JSON");
        assert_eq!(encoded["schema_version"], "pitgun.racing-driver/v2");
        assert_eq!(
            serde_json::to_value(RacingDrivingMode::Attack).expect("mode JSON"),
            "attack"
        );
        assert_eq!(
            serde_json::from_value::<RacingDriverResourceV2>(encoded).expect("driver round trip"),
            driver
        );
    }

    #[test]
    fn instruction_timeline_uses_one_balanced_default_and_canonical_order() {
        let profile = valid_instruction_profile();
        assert_eq!(profile.default_mode, RacingDrivingMode::Balanced);

        let timeline = valid_timeline();
        timeline
            .validate_for_session(&profile, &competitors(), 5, 20)
            .expect("valid canonical timeline");

        let encoded = serde_json::to_value(&timeline).expect("timeline JSON");
        assert_eq!(
            encoded["schema_version"],
            "pitgun.racing-driver-instruction-timeline/v1"
        );
        assert_eq!(encoded["events"][0]["mode"], "manage");
        assert_eq!(encoded["events"][1]["mode"], "attack");
    }

    #[test]
    fn instruction_timeline_fails_closed_on_wire_extensions() {
        let unknown_profile_field = serde_json::json!({
            "schema_version": "pitgun.racing-driver-instruction-profile/v1",
            "default_mode": "balanced",
            "boundary_granularity": "lap_start",
            "max_events_per_session": 8,
            "player_default_mode": "attack"
        });
        assert!(
            serde_json::from_value::<RacingDriverInstructionProfileV1>(unknown_profile_field)
                .is_err()
        );

        let unknown_profile_version = serde_json::json!({
            "schema_version": "pitgun.racing-driver-instruction-profile/v2",
            "default_mode": "balanced",
            "boundary_granularity": "lap_start",
            "max_events_per_session": 8
        });
        assert!(
            serde_json::from_value::<RacingDriverInstructionProfileV1>(unknown_profile_version)
                .is_err()
        );

        let unknown_timeline_field = serde_json::json!({
            "schema_version": "pitgun.racing-driver-instruction-timeline/v1",
            "events": [],
            "source": "player"
        });
        assert!(
            serde_json::from_value::<RacingDriverInstructionTimelineV1>(unknown_timeline_field)
                .is_err()
        );

        let unknown_version = serde_json::json!({
            "schema_version": "pitgun.racing-driver-instruction-timeline/v2",
            "events": []
        });
        assert!(
            serde_json::from_value::<RacingDriverInstructionTimelineV1>(unknown_version).is_err()
        );
    }

    #[test]
    fn instruction_timeline_rejects_invalid_sequence_order_and_duplicates() {
        let profile = valid_instruction_profile();

        let mut sequence_gap = valid_timeline();
        sequence_gap.events[1].sequence = 7;
        assert_eq!(
            sequence_gap.validate_for_session(&profile, &competitors(), 5, 20),
            Err(
                RacingDriverContractError::NonContiguousInstructionSequence {
                    expected: 1,
                    actual: 7
                }
            )
        );

        let mut noncanonical = valid_timeline();
        noncanonical.events.swap(0, 1);
        noncanonical.events[0].sequence = 0;
        noncanonical.events[1].sequence = 1;
        assert_eq!(
            noncanonical.validate_for_session(&profile, &competitors(), 5, 20),
            Err(RacingDriverContractError::NonCanonicalInstructionOrder { sequence: 1 })
        );

        let duplicate = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: vec![
                instruction(0, "player", 1, 0, RacingDrivingMode::Attack),
                instruction(1, "player", 1, 0, RacingDrivingMode::Balanced),
            ],
        };
        assert_eq!(
            duplicate.validate_for_session(&profile, &competitors(), 5, 20),
            Err(RacingDriverContractError::DuplicateInstructionBoundary {
                sequence: 1,
                competitor_id: "player".to_string()
            })
        );
    }

    #[test]
    fn instruction_timeline_rejects_invalid_subjects_and_boundaries() {
        let profile = valid_instruction_profile();

        let unknown = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: vec![instruction(0, "ghost", 1, 0, RacingDrivingMode::Attack)],
        };
        assert!(matches!(
            unknown.validate_for_session(&profile, &competitors(), 5, 20),
            Err(RacingDriverContractError::UnknownInstructionCompetitor { .. })
        ));

        let initial = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: vec![instruction(0, "player", 0, 0, RacingDrivingMode::Attack)],
        };
        assert_eq!(
            initial.validate_for_session(&profile, &competitors(), 5, 20),
            Err(RacingDriverContractError::InstructionAtInitialBoundary { sequence: 0 })
        );

        let unsupported_segment = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: vec![instruction(0, "player", 0, 1, RacingDrivingMode::Attack)],
        };
        assert!(matches!(
            unsupported_segment.validate_for_session(&profile, &competitors(), 5, 20),
            Err(RacingDriverContractError::UnsupportedInstructionBoundary { .. })
        ));

        let outside = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: vec![instruction(0, "player", 5, 0, RacingDrivingMode::Attack)],
        };
        assert_eq!(
            outside.validate_for_session(&profile, &competitors(), 5, 20),
            Err(RacingDriverContractError::InstructionBoundaryOutOfRange { sequence: 0 })
        );
    }

    #[test]
    fn instruction_profile_bounds_event_count_and_can_allow_segments() {
        let mut profile = valid_instruction_profile();
        profile.max_events_per_session = 2;
        assert_eq!(
            valid_timeline().validate_for_session(&profile, &competitors(), 5, 20),
            Err(RacingDriverContractError::TooManyInstructionEvents {
                maximum: 2,
                actual: 3
            })
        );

        profile.max_events_per_session = 8;
        profile.boundary_granularity = RacingDriverInstructionBoundaryGranularityV1::Segment;
        let segment_timeline = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: vec![instruction(0, "player", 0, 1, RacingDrivingMode::Attack)],
        };
        segment_timeline
            .validate_for_session(&profile, &competitors(), 5, 20)
            .expect("segment-enabled profile accepts a post-start segment");
    }
}

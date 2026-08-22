//! Versioned Racing driver-control contracts.

use std::fmt;

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

/// Explicit commitment requested for one competitor and session.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RacingDrivingMode {
    Manage,
    Balanced,
    Attack,
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
}

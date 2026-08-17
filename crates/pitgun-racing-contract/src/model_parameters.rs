//! Immutable, model-scoped Racing parameter-resource contracts.
//!
//! This module owns wire semantics only. It deliberately does not apply game
//! progression, execute physical equations, or select a mutable catalog
//! release.

use std::fmt;

use pitgun_contract::{
    ArtifactIdentity, Digest, Identifier, SemanticVersion, canonical_json_digest,
};
use serde::{Deserialize, Serialize};

const RACING_MODEL_ID: &str = "pitgun.racing";
const RACING_MODEL_V2_VERSION: &str = "2.0.0";
const RESOURCE_ID_PREFIX: &str = "pitgun.racing.model-parameters.";

const MIN_DEVELOPMENT_POINTS_CAP: f64 = 1.0;
const MAX_DEVELOPMENT_POINTS_CAP: f64 = 100.0;
const MIN_POSITIVE_MULTIPLIER: f64 = 0.1;
const MAX_MULTIPLIER: f64 = 4.0;
const MIN_GAIN: f64 = 0.0;
const MAX_GAIN: f64 = 4.0;

/// Wire version of an immutable Racing model-parameter resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingModelParametersVersion {
    /// First reviewed resource boundary for Racing Model V2 compatibility.
    #[serde(rename = "pitgun.racing-model-parameters/v1")]
    V1,
}

/// Intended governance status of one parameter resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingModelParametersPurpose {
    /// Byte-compatible representation of the historical compiled V2 values.
    #[serde(rename = "model-v2-compatibility")]
    ModelV2Compatibility,
    /// Offline V2 candidate that must not become production by mutable update.
    #[serde(rename = "model-v2-offline-candidate")]
    ModelV2OfflineCandidate,
}

/// Stable semantic identity of the resource before its catalog digest is added.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingModelParametersIdentityV1 {
    /// Stable resource identifier within the Racing domain.
    pub id: Identifier,
    /// Exact parameter semantics version.
    pub version: SemanticVersion,
}

/// Exact executable-model semantics supported by this resource.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingModelCompatibilityV1 {
    /// Stable model family identifier.
    pub id: Identifier,
    /// Exact compatible model version.
    pub version: SemanticVersion,
}

/// Mapping from game development points to physical multipliers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDevelopmentResolutionV1 {
    /// Per-axis point cap, in development points per axis.
    pub points_cap_per_axis: f64,
    /// Dimensionless aerodynamic-area gain reached at the cap.
    pub aerodynamic_area_gain_at_cap: f64,
    /// Dimensionless chassis-grip gain reached at the cap.
    pub chassis_grip_gain_at_cap: f64,
    /// Dimensionless cooling multiplier at zero cooling points.
    pub cooling_base_multiplier: f64,
    /// Dimensionless cooling gain reached at the cap.
    pub cooling_gain_at_cap: f64,
    /// Dimensionless torque-curve gain reached at the cap.
    pub engine_torque_gain_at_cap: f64,
}

/// Mapping from normalized player setup controls to physical multipliers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingSetupResponseV1 {
    /// Dimensionless drag-area multiplier at minimum aero setup.
    pub drag_area_base_multiplier: f64,
    /// Dimensionless drag-area gain over the normalized aero slider.
    pub drag_area_slider_gain: f64,
    /// Dimensionless downforce-area multiplier at minimum aero setup.
    pub downforce_area_base_multiplier: f64,
    /// Dimensionless downforce-area gain over the normalized aero slider.
    pub downforce_area_slider_gain: f64,
    /// Dimensionless common gear-ratio multiplier at minimum gearing setup.
    pub gear_ratio_base_multiplier: f64,
    /// Dimensionless reduction over the normalized gearing slider.
    pub gear_ratio_slider_reduction: f64,
}

/// Model-V2 aerodynamic-state response coefficients.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingAerodynamicStateResponseV1 {
    /// Dimensionless drag/downforce scale at the full-straight state.
    pub straight_multiplier: f64,
    /// Dimensionless drag/downforce scale at the full-corner state.
    pub corner_multiplier: f64,
}

/// Strict immutable resource for the reviewed Racing Model V2 parameter boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingModelParametersV1 {
    /// Exact resource schema version.
    pub schema_version: RacingModelParametersVersion,
    /// Semantic resource identity; the catalog index adds the content digest.
    pub identity: RacingModelParametersIdentityV1,
    /// Exact executable-model compatibility declaration.
    pub compatible_model: RacingModelCompatibilityV1,
    /// Governance status separating replay compatibility from experiments.
    pub purpose: RacingModelParametersPurpose,
    /// Game progression to physical response.
    pub development_resolution: RacingDevelopmentResolutionV1,
    /// Player setup to physical response.
    pub setup_response: RacingSetupResponseV1,
    /// Reviewed equation coefficients retained by Racing Model V2.
    pub aerodynamic_state_response: RacingAerodynamicStateResponseV1,
}

/// Structural or semantic failure in a Racing model-parameter resource.
#[derive(Clone, Debug, PartialEq)]
pub enum RacingModelParametersError {
    /// Resource identity is outside the governed Racing namespace.
    InvalidResourceIdentity(String),
    /// V1 supports only the exact historical Racing Model V2 semantics.
    UnsupportedModel { id: String, version: String },
    /// A numeric parameter is non-finite or outside its V1 admissibility range.
    InvalidParameter {
        field: &'static str,
        value: f64,
        constraint: &'static str,
    },
    /// Two individually valid parameters violate a joint invariant.
    InvalidRelationship(&'static str),
    /// Canonical resource bytes could not be produced.
    Canonicalization(String),
}

impl fmt::Display for RacingModelParametersError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidResourceIdentity(id) => write!(
                formatter,
                "Racing model-parameter resource ID is outside the governed namespace: {id}"
            ),
            Self::UnsupportedModel { id, version } => {
                write!(
                    formatter,
                    "unsupported Racing model compatibility: {id}@{version}"
                )
            }
            Self::InvalidParameter {
                field,
                value,
                constraint,
            } => write!(formatter, "invalid {field}={value}: expected {constraint}"),
            Self::InvalidRelationship(reason) => formatter.write_str(reason),
            Self::Canonicalization(reason) => {
                write!(
                    formatter,
                    "cannot canonicalize Racing model parameters: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for RacingModelParametersError {}

impl RacingModelParametersV1 {
    /// Validates namespace, exact model compatibility, numeric bounds and joints.
    pub fn validate(&self) -> Result<(), RacingModelParametersError> {
        if !self.identity.id.as_str().starts_with(RESOURCE_ID_PREFIX) {
            return Err(RacingModelParametersError::InvalidResourceIdentity(
                self.identity.id.to_string(),
            ));
        }
        if self.compatible_model.id.as_str() != RACING_MODEL_ID
            || self.compatible_model.version.to_string() != RACING_MODEL_V2_VERSION
        {
            return Err(RacingModelParametersError::UnsupportedModel {
                id: self.compatible_model.id.to_string(),
                version: self.compatible_model.version.to_string(),
            });
        }

        let development = self.development_resolution;
        validate_inclusive(
            "development_resolution.points_cap_per_axis",
            development.points_cap_per_axis,
            MIN_DEVELOPMENT_POINTS_CAP,
            MAX_DEVELOPMENT_POINTS_CAP,
            "a finite value in [1, 100] development points per axis",
        )?;
        for (field, value) in [
            (
                "development_resolution.aerodynamic_area_gain_at_cap",
                development.aerodynamic_area_gain_at_cap,
            ),
            (
                "development_resolution.chassis_grip_gain_at_cap",
                development.chassis_grip_gain_at_cap,
            ),
            (
                "development_resolution.cooling_gain_at_cap",
                development.cooling_gain_at_cap,
            ),
            (
                "development_resolution.engine_torque_gain_at_cap",
                development.engine_torque_gain_at_cap,
            ),
        ] {
            validate_inclusive(
                field,
                value,
                MIN_GAIN,
                MAX_GAIN,
                "a finite dimensionless gain in [0, 4]",
            )?;
        }
        validate_inclusive(
            "development_resolution.cooling_base_multiplier",
            development.cooling_base_multiplier,
            MIN_POSITIVE_MULTIPLIER,
            MAX_MULTIPLIER,
            "a finite dimensionless multiplier in [0.1, 4]",
        )?;

        let setup = self.setup_response;
        for (field, value) in [
            (
                "setup_response.drag_area_base_multiplier",
                setup.drag_area_base_multiplier,
            ),
            (
                "setup_response.downforce_area_base_multiplier",
                setup.downforce_area_base_multiplier,
            ),
            (
                "setup_response.gear_ratio_base_multiplier",
                setup.gear_ratio_base_multiplier,
            ),
        ] {
            validate_inclusive(
                field,
                value,
                MIN_POSITIVE_MULTIPLIER,
                MAX_MULTIPLIER,
                "a finite dimensionless multiplier in [0.1, 4]",
            )?;
        }
        for (field, value) in [
            (
                "setup_response.drag_area_slider_gain",
                setup.drag_area_slider_gain,
            ),
            (
                "setup_response.downforce_area_slider_gain",
                setup.downforce_area_slider_gain,
            ),
            (
                "setup_response.gear_ratio_slider_reduction",
                setup.gear_ratio_slider_reduction,
            ),
        ] {
            validate_inclusive(
                field,
                value,
                MIN_GAIN,
                MAX_GAIN,
                "a finite dimensionless response in [0, 4]",
            )?;
        }
        if setup.gear_ratio_slider_reduction >= setup.gear_ratio_base_multiplier {
            return Err(RacingModelParametersError::InvalidRelationship(
                "gear-ratio slider reduction must remain below its base multiplier",
            ));
        }

        let aerodynamic = self.aerodynamic_state_response;
        for (field, value) in [
            (
                "aerodynamic_state_response.straight_multiplier",
                aerodynamic.straight_multiplier,
            ),
            (
                "aerodynamic_state_response.corner_multiplier",
                aerodynamic.corner_multiplier,
            ),
        ] {
            validate_inclusive(
                field,
                value,
                MIN_POSITIVE_MULTIPLIER,
                MAX_MULTIPLIER,
                "a finite dimensionless multiplier in [0.1, 4]",
            )?;
        }
        Ok(())
    }

    /// Fails closed unless the run selects the exact compatible model semantics.
    pub fn validate_for_model(
        &self,
        model: &ArtifactIdentity,
    ) -> Result<(), RacingModelParametersError> {
        self.validate()?;
        if model.id != self.compatible_model.id || model.version != self.compatible_model.version {
            return Err(RacingModelParametersError::UnsupportedModel {
                id: model.id.to_string(),
                version: model.version.to_string(),
            });
        }
        Ok(())
    }

    /// Returns the canonical content digest used by the immutable catalog index.
    pub fn canonical_digest(&self) -> Result<Digest, RacingModelParametersError> {
        self.validate()?;
        canonical_json_digest(self)
            .map_err(|error| RacingModelParametersError::Canonicalization(error.to_string()))
    }
}

fn validate_inclusive(
    field: &'static str,
    value: f64,
    minimum: f64,
    maximum: f64,
    constraint: &'static str,
) -> Result<(), RacingModelParametersError> {
    if !value.is_finite() || value < minimum || value > maximum {
        return Err(RacingModelParametersError::InvalidParameter {
            field,
            value,
            constraint,
        });
    }
    Ok(())
}

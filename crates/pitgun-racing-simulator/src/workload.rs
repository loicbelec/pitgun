//! Statically linked Racing workload for the deterministic Pitgun runtime.

use pitgun_contract::{ArtifactIdentity, ContractVersion};
use pitgun_runtime::{ExecutionContext, LinkedWorkload, WorkloadExecution};

use crate::evidence::{RacingEvidenceError, RacingRunEvidenceV1};
use crate::{
    CurvatureAeroResponse, RaceOutput, RacingCatalogSnapshot, RunRaceInput, RunRaceRequest,
    resolve_catalog_tuning_response, run_race, run_race_with_catalog_and_model_response,
    run_race_with_catalog_and_v3_thermal_family_profile,
};

const RACING_MODEL_V1_MANIFEST: &[u8] = b"pitgun.racing:model:1.0.0:conformance-vector";
const RACING_MODEL_V2_MANIFEST: &[u8] = b"pitgun.racing:model:2.0.0:continuous-curvature-v1";
const RACING_MODEL_V3_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.9.0:resolved-vehicle:per-segment-v1:aggregate-contact-v2:mechanical-controls-v1:aero-efficiency-v1:development-resolution-v1:transmission-resolution-v1:zero-downforce-v1:first-stint-tire-v1:power-based-fuel-mass-v1:compound-degradation-v1";
const RACING_MODEL_V3_THERMAL_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.10.0:resolved-vehicle:per-segment-v1:aggregate-contact-v2:mechanical-controls-v1:aero-efficiency-v1:development-resolution-v1:transmission-resolution-v1:zero-downforce-v1:first-stint-tire-v1:power-based-fuel-mass-v1:compound-degradation-v1:engine-thermal-resolution-v1";
const RACING_MODEL_V3_FUEL_MASS_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.8.0:resolved-vehicle:per-segment-v1:aggregate-contact-v1:mechanical-controls-v1:aero-efficiency-v1:development-resolution-v1:transmission-resolution-v1:zero-downforce-v1:first-stint-tire-v1:power-based-fuel-mass-v1";
const RACING_MODEL_V3_FIDELITY_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.7.0:resolved-vehicle:per-segment-v1:aggregate-contact-v1:mechanical-controls-v1:aero-efficiency-v1:development-resolution-v1:transmission-resolution-v1:zero-downforce-v1:first-stint-tire-v1";
const RACING_MODEL_V3_TRANSMISSION_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.6.0:resolved-vehicle:per-segment-v1:aggregate-contact-v1:mechanical-controls-v1:aero-efficiency-v1:development-resolution-v1:transmission-resolution-v1";
const RACING_MODEL_V3_DEVELOPMENT_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.5.0:resolved-vehicle:per-segment-v1:aggregate-contact-v1:mechanical-controls-v1:aero-efficiency-v1:development-resolution-v1";
const RACING_MODEL_V3_AERO_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.4.0:resolved-vehicle:per-segment-v1:aggregate-contact-v1:mechanical-controls-v1:aero-efficiency-v1";
const RACING_MODEL_V3_MECHANICAL_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.3.0:resolved-vehicle:per-segment-v1:aggregate-contact-v1:mechanical-controls-v1";

/// Returns the exact logical Racing model identity authorized by V1 services.
#[must_use]
pub fn racing_model_v1_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing".parse().expect("static Racing model id"),
        version: "1.0.0".parse().expect("static Racing model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V1_MANIFEST),
    }
}

/// Returns the exact logical identity of the continuous-curvature Racing model.
#[must_use]
pub fn racing_model_v2_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing".parse().expect("static Racing model id"),
        version: "2.0.0".parse().expect("static Racing model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V2_MANIFEST),
    }
}

/// Returns the non-production identity of the current Model V3 tire slice.
///
/// This candidate namespace prevents an incomplete V3 from being mistaken for
/// the future published `pitgun.racing@3.0.0` workload.
#[must_use]
pub fn racing_model_v3_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.9.0"
            .parse()
            .expect("static Racing V3 candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_CANDIDATE_MANIFEST),
    }
}

/// Returns the non-production identity of the explicit thermal-parameter slice.
///
/// Candidate 0.10 keeps the 0.9 equations as its baseline while moving every
/// reviewed engine heat, inertia, rejection, threshold and derating response
/// behind a checksummed experiment profile.
#[must_use]
pub fn racing_model_v3_thermal_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.10.0"
            .parse()
            .expect("static Racing V3 thermal candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_THERMAL_CANDIDATE_MANIFEST),
    }
}

/// Returns the immutable identity of the preceding fuel and mass slice.
#[must_use]
pub fn racing_model_v3_fuel_mass_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.8.0"
            .parse()
            .expect("static Racing V3 fuel-mass candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_FUEL_MASS_CANDIDATE_MANIFEST),
    }
}

/// Returns the immutable identity of the preceding active-vehicle and tire slice.
#[must_use]
pub fn racing_model_v3_fidelity_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.7.0"
            .parse()
            .expect("static Racing V3 fidelity candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_FIDELITY_CANDIDATE_MANIFEST),
    }
}

/// Returns the immutable identity of the preceding transmission-resolution slice.
///
/// It remains executable only for replaying the local V4 screening and first
/// held-out validation profile.
#[must_use]
pub fn racing_model_v3_transmission_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.6.0"
            .parse()
            .expect("static Racing V3 transmission candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(
            RACING_MODEL_V3_TRANSMISSION_CANDIDATE_MANIFEST,
        ),
    }
}

/// Returns the immutable identity of the preceding development-resolution slice.
///
/// It remains executable only for replaying the local V3 screening profile.
#[must_use]
pub fn racing_model_v3_development_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.5.0"
            .parse()
            .expect("static Racing V3 development candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_DEVELOPMENT_CANDIDATE_MANIFEST),
    }
}

/// Returns the immutable identity of the preceding aerodynamic-efficiency slice.
///
/// It remains executable only for replaying the local V2 screening profile.
#[must_use]
pub fn racing_model_v3_aero_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.4.0"
            .parse()
            .expect("static Racing V3 aero candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_AERO_CANDIDATE_MANIFEST),
    }
}

/// Returns the immutable identity of the preceding mechanical-controls slice.
///
/// It remains executable only for replaying the local V1 screening profile.
#[must_use]
pub fn racing_model_v3_mechanical_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.3.0"
            .parse()
            .expect("static Racing V3 mechanical candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_MECHANICAL_CANDIDATE_MANIFEST),
    }
}

/// Resolves one supported Racing model version to its exact immutable identity.
pub fn racing_model_identity_for_version(version: &str) -> Result<ArtifactIdentity, String> {
    match version {
        "1.0.0" => Ok(racing_model_v1_identity()),
        "2.0.0" => Ok(racing_model_v2_identity()),
        "0.10.0" => Ok(racing_model_v3_thermal_candidate_identity()),
        _ => Err(format!(
            "unsupported Racing model version {version:?}; expected 1.0.0, 2.0.0 or 0.10.0"
        )),
    }
}

/// Statically linked adapter for one exact Racing model identity.
#[derive(Clone, Debug)]
pub struct RacingWorkload {
    model: ArtifactIdentity,
    catalog: Option<RacingCatalogSnapshot>,
    curvature_response: CurvatureAeroResponse,
}

impl RacingWorkload {
    /// Creates the adapter for the published Racing model V1 identity.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            model: racing_model_v1_identity(),
            catalog: None,
            curvature_response: CurvatureAeroResponse::LegacyBinary,
        }
    }

    /// Creates the V1 adapter pinned to a validated immutable catalog snapshot.
    #[must_use]
    pub fn with_catalog(catalog: RacingCatalogSnapshot) -> Self {
        Self {
            model: racing_model_v1_identity(),
            catalog: Some(catalog),
            curvature_response: CurvatureAeroResponse::LegacyBinary,
        }
    }

    /// Creates the V2 adapter pinned to its immutable catalog snapshot.
    #[must_use]
    pub fn v2_with_catalog(catalog: RacingCatalogSnapshot) -> Self {
        Self {
            model: racing_model_v2_identity(),
            catalog: Some(catalog),
            curvature_response: CurvatureAeroResponse::ContinuousV1,
        }
    }

    /// Creates the non-production V3 thermal adapter pinned to its exact catalog.
    #[must_use]
    pub fn v3_thermal_with_catalog(catalog: RacingCatalogSnapshot) -> Self {
        Self {
            model: racing_model_v3_thermal_candidate_identity(),
            catalog: Some(catalog),
            curvature_response: CurvatureAeroResponse::ContinuousV1,
        }
    }

    /// Selects the statically linked workload for one exact model/catalog pair.
    pub fn for_model(
        model: &ArtifactIdentity,
        catalog: RacingCatalogSnapshot,
    ) -> Result<Self, String> {
        catalog
            .manifest()
            .compatibility
            .validate_for(model, ContractVersion::V1)
            .map_err(|error| format!("Racing model/catalog incompatibility: {error}"))?;

        if *model == racing_model_v1_identity() {
            Ok(Self::with_catalog(catalog))
        } else if *model == racing_model_v2_identity() {
            Ok(Self::v2_with_catalog(catalog))
        } else if *model == racing_model_v3_thermal_candidate_identity() {
            Ok(Self::v3_thermal_with_catalog(catalog))
        } else {
            Err(format!(
                "unsupported Racing model identity {}@{} {}",
                model.id, model.version, model.digest
            ))
        }
    }
}

/// Failure produced while executing or projecting evidence for Racing.
#[derive(Debug, thiserror::Error)]
pub enum RacingWorkloadError {
    /// The Racing Simulator rejected the request.
    #[error("Racing simulation failed: {0}")]
    Simulation(String),
    /// Racing output could not be projected into canonical evidence.
    #[error("Racing evidence failed: {0}")]
    Evidence(#[from] RacingEvidenceError),
}

impl LinkedWorkload for RacingWorkload {
    type Input = RunRaceInput;
    type Output = RaceOutput;
    type Evidence = RacingRunEvidenceV1;
    type Error = RacingWorkloadError;

    fn model_identity(&self) -> &ArtifactIdentity {
        &self.model
    }

    fn execute(
        &self,
        context: &ExecutionContext<'_>,
        input: Self::Input,
    ) -> Result<WorkloadExecution<Self::Output, Self::Evidence>, Self::Error> {
        let era = input.era;
        let hz = input.hz;
        let request = RunRaceRequest {
            input,
            seed: context.seed().get(),
            era: Some(era),
            hz: Some(hz),
        };
        let output = match &self.catalog {
            Some(catalog) => {
                catalog
                    .manifest()
                    .compatibility
                    .validate_for(&self.model, pitgun_contract::ContractVersion::V1)
                    .map_err(|error| RacingWorkloadError::Simulation(error.to_string()))?;
                if self.model == racing_model_v3_thermal_candidate_identity() {
                    let thermal_profile = catalog.thermal_family_profile().ok_or_else(|| {
                        RacingWorkloadError::Simulation(
                            "Model V3 thermal catalog has no reviewed thermal-family profile"
                                .to_string(),
                        )
                    })?;
                    run_race_with_catalog_and_v3_thermal_family_profile(
                        request,
                        catalog,
                        thermal_profile,
                    )
                } else {
                    let tuning_response =
                        resolve_catalog_tuning_response(catalog, Some(&self.model))
                            .map_err(RacingWorkloadError::Simulation)?;
                    run_race_with_catalog_and_model_response(
                        request,
                        catalog,
                        &tuning_response,
                        self.curvature_response,
                    )
                }
            }
            None => run_race(request),
        }
        .map_err(RacingWorkloadError::Simulation)?;
        let evidence = RacingRunEvidenceV1::from_race_output(&output)?;

        Ok(WorkloadExecution { output, evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RacingWorkload, racing_model_identity_for_version, racing_model_v2_identity,
        racing_model_v3_aero_candidate_identity, racing_model_v3_candidate_identity,
        racing_model_v3_development_candidate_identity,
        racing_model_v3_fidelity_candidate_identity, racing_model_v3_fuel_mass_candidate_identity,
        racing_model_v3_mechanical_candidate_identity, racing_model_v3_thermal_candidate_identity,
        racing_model_v3_transmission_candidate_identity,
    };
    use pitgun_runtime::LinkedWorkload;

    #[test]
    fn racing_workload_v1_has_the_published_model_identity() {
        let model = RacingWorkload::v1().model_identity().clone();

        assert_eq!(model.id.to_string(), "pitgun.racing");
        assert_eq!(model.version.to_string(), "1.0.0");
        assert_eq!(
            model.digest.to_string(),
            "sha256:03541bcc24f946d11071e6fb67915ec5d429dce63362d456aba2c3d339a3fe38"
        );
    }

    #[test]
    fn racing_workload_v2_has_a_distinct_published_model_identity() {
        let model = racing_model_v2_identity();

        assert_eq!(model.id.to_string(), "pitgun.racing");
        assert_eq!(model.version.to_string(), "2.0.0");
        assert_eq!(
            model.digest.to_string(),
            "sha256:a372f990c320d10207220f98ca4bf677607fc5c13918c73b47dfbb8949b106d2"
        );
        assert_ne!(model, RacingWorkload::v1().model_identity().clone());
    }

    #[test]
    fn model_version_selection_is_exact_and_fail_closed() {
        assert_eq!(
            racing_model_identity_for_version("2.0.0").expect("supported V2"),
            racing_model_v2_identity()
        );
        assert_eq!(
            racing_model_identity_for_version("0.10.0").expect("supported V3 thermal candidate"),
            racing_model_v3_thermal_candidate_identity()
        );
        assert!(racing_model_identity_for_version("2").is_err());
        assert!(racing_model_identity_for_version("3.0.0").is_err());
    }

    #[test]
    fn v3_candidate_identity_cannot_masquerade_as_a_published_model() {
        let candidate = racing_model_v3_candidate_identity();

        assert_eq!(candidate.id.to_string(), "pitgun.racing-v3-candidate");
        assert_eq!(candidate.version.to_string(), "0.9.0");
        assert_eq!(
            candidate.digest.to_string(),
            "sha256:d8767c911912c1ae19cf50f8bb2c6455f7308d83a762d6192f4ef090ef199d99"
        );
        assert_ne!(candidate, racing_model_v2_identity());
        assert!(racing_model_identity_for_version("0.9.0").is_err());

        let thermal = racing_model_v3_thermal_candidate_identity();
        assert_eq!(thermal.version.to_string(), "0.10.0");
        assert_eq!(
            thermal.digest.to_string(),
            "sha256:cc1394a1ba52d83ddb9be6f6272729c29f87944969c41a21991d43997379e5cd"
        );
        assert_ne!(candidate, thermal);

        let fuel_mass = racing_model_v3_fuel_mass_candidate_identity();
        assert_eq!(fuel_mass.version.to_string(), "0.8.0");
        assert_eq!(
            fuel_mass.digest.to_string(),
            "sha256:01c7d8abbe33e8dd7afa87a5ba13668f579f746de8a781ed242e2ac73e0bed6e"
        );

        let fidelity = racing_model_v3_fidelity_candidate_identity();
        assert_eq!(fidelity.version.to_string(), "0.7.0");
        assert_eq!(
            fidelity.digest.to_string(),
            "sha256:fa8f557fde751d8d38e52bf0d5961ae2b06b4dd1915825c6deb1d869592f0afa"
        );
        assert_ne!(candidate, fidelity);

        let transmission = racing_model_v3_transmission_candidate_identity();
        assert_eq!(transmission.version.to_string(), "0.6.0");
        assert_eq!(
            transmission.digest.to_string(),
            "sha256:ecb7bd48bb0ed556f3b6e9f20f15f12646eca0ef70a57afd05de8e59f435f936"
        );
        assert_ne!(candidate, transmission);

        let development = racing_model_v3_development_candidate_identity();
        assert_eq!(development.version.to_string(), "0.5.0");
        assert_eq!(
            development.digest.to_string(),
            "sha256:359720d98e93dfdd7ed51f9bdf2ce52fc593a00f76024bb1f90839c90f27dc16"
        );
        assert_ne!(candidate, development);

        let aero_candidate = racing_model_v3_aero_candidate_identity();
        assert_eq!(aero_candidate.version.to_string(), "0.4.0");
        assert_eq!(
            aero_candidate.digest.to_string(),
            "sha256:849d109d7a15345f58d2cdd5f33f62e5f429ff49cb7f14989775f50c1a050598"
        );
        assert_ne!(candidate, aero_candidate);

        let mechanical = racing_model_v3_mechanical_candidate_identity();
        assert_eq!(mechanical.version.to_string(), "0.3.0");
        assert_eq!(
            mechanical.digest.to_string(),
            "sha256:79a262d7c1625ca577627e0d134a744284f6aabd15ee683133c572dd4af4e35c"
        );
        assert_ne!(mechanical, candidate);
    }
}

//! Statically linked Racing workload for the deterministic Pitgun runtime.

use pitgun_contract::{ArtifactIdentity, ContractVersion};
use pitgun_runtime::{ExecutionContext, LinkedWorkload, WorkloadExecution};

use crate::evidence::{RacingEvidenceError, RacingRunEvidenceV1};
use crate::{
    CurvatureAeroResponse, RaceOutput, RacingCatalogSnapshot, RunRaceInput, RunRaceRequest,
    resolve_catalog_tuning_response, run_race, run_race_with_catalog_and_model_response,
};

const RACING_MODEL_V1_MANIFEST: &[u8] = b"pitgun.racing:model:1.0.0:conformance-vector";
const RACING_MODEL_V2_MANIFEST: &[u8] = b"pitgun.racing:model:2.0.0:continuous-curvature-v1";
const RACING_MODEL_V3_CANDIDATE_MANIFEST: &[u8] =
    b"pitgun.racing-v3-candidate:model:0.1.0:resolved-vehicle:per-segment-v1";

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

/// Returns the non-production identity of the first Model V3 mechanical slice.
///
/// This candidate namespace prevents an incomplete V3 from being mistaken for
/// the future published `pitgun.racing@3.0.0` workload.
#[must_use]
pub fn racing_model_v3_candidate_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate"
            .parse()
            .expect("static Racing V3 candidate model id"),
        version: "0.1.0"
            .parse()
            .expect("static Racing V3 candidate model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V3_CANDIDATE_MANIFEST),
    }
}

/// Resolves one supported Racing model version to its exact immutable identity.
pub fn racing_model_identity_for_version(version: &str) -> Result<ArtifactIdentity, String> {
    match version {
        "1.0.0" => Ok(racing_model_v1_identity()),
        "2.0.0" => Ok(racing_model_v2_identity()),
        _ => Err(format!(
            "unsupported Racing model version {version:?}; expected 1.0.0 or 2.0.0"
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
                let tuning_response = resolve_catalog_tuning_response(catalog, Some(&self.model))
                    .map_err(RacingWorkloadError::Simulation)?;
                run_race_with_catalog_and_model_response(
                    request,
                    catalog,
                    &tuning_response,
                    self.curvature_response,
                )
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
        racing_model_v3_candidate_identity,
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
        assert!(racing_model_identity_for_version("2").is_err());
        assert!(racing_model_identity_for_version("3.0.0").is_err());
    }

    #[test]
    fn v3_candidate_identity_cannot_masquerade_as_a_published_model() {
        let candidate = racing_model_v3_candidate_identity();

        assert_eq!(candidate.id.to_string(), "pitgun.racing-v3-candidate");
        assert_eq!(candidate.version.to_string(), "0.1.0");
        assert_ne!(candidate, racing_model_v2_identity());
        assert!(racing_model_identity_for_version("0.1.0").is_err());
    }
}

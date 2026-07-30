//! Statically linked Racing workload for the deterministic Pitgun runtime.

use pitgun_contract::ArtifactIdentity;
use pitgun_runtime::{ExecutionContext, LinkedWorkload, WorkloadExecution};

use crate::evidence::{RacingEvidenceError, RacingRunEvidenceV1};
use crate::{
    RaceOutput, RacingCatalogSnapshot, RunRaceInput, RunRaceRequest, run_race,
    run_race_with_catalog,
};

const RACING_MODEL_V1_MANIFEST: &[u8] = b"pitgun.racing:model:1.0.0:conformance-vector";

/// Returns the exact logical Racing model identity authorized by V1 services.
#[must_use]
pub fn racing_model_v1_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing".parse().expect("static Racing model id"),
        version: "1.0.0".parse().expect("static Racing model version"),
        digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V1_MANIFEST),
    }
}

/// Statically linked adapter for one exact Racing model identity.
#[derive(Clone, Debug)]
pub struct RacingWorkload {
    model: ArtifactIdentity,
    catalog: Option<RacingCatalogSnapshot>,
}

impl RacingWorkload {
    /// Creates the adapter for the published Racing model V1 identity.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            model: racing_model_v1_identity(),
            catalog: None,
        }
    }

    /// Creates the V1 adapter pinned to a validated immutable catalog snapshot.
    #[must_use]
    pub fn with_catalog(catalog: RacingCatalogSnapshot) -> Self {
        Self {
            model: racing_model_v1_identity(),
            catalog: Some(catalog),
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
            Some(catalog) => run_race_with_catalog(request, catalog),
            None => run_race(request),
        }
        .map_err(RacingWorkloadError::Simulation)?;
        let evidence = RacingRunEvidenceV1::from_race_output(&output)?;

        Ok(WorkloadExecution { output, evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::RacingWorkload;
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
}

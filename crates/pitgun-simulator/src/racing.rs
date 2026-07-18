//! Transitional public boundary for the Racing reference workload.
//!
//! Race orchestration still lives in `pitgun-solver` and is scheduled to move
//! under this crate in #39. Callers use this module so that migration does not
//! change their domain-facing API.

use pitgun_contract::ArtifactIdentity;
use pitgun_runtime::{ExecutionContext, LinkedWorkload, WorkloadExecution};

pub use pitgun_solver::evidence::{RacingEvidenceError, RacingRunEvidenceV1};
pub use pitgun_solver::{RaceOutput, RunRaceInput, RunRaceRequest};

const RACING_MODEL_V1_MANIFEST: &[u8] = b"pitgun.racing:model:1.0.0:conformance-vector";

/// Statically linked adapter for one exact Racing model identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RacingWorkload {
    model: ArtifactIdentity,
}

impl RacingWorkload {
    /// Creates the adapter for the published Racing model V1 identity.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            model: ArtifactIdentity {
                id: "pitgun.racing".parse().expect("static Racing model id"),
                version: "1.0.0".parse().expect("static Racing model version"),
                digest: pitgun_contract::Digest::from_bytes(RACING_MODEL_V1_MANIFEST),
            },
        }
    }
}

/// Failure produced while executing or projecting evidence for Racing.
#[derive(Debug, thiserror::Error)]
pub enum RacingWorkloadError {
    /// The transitional Racing implementation rejected the request.
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
        let output = run_race(RunRaceRequest {
            input,
            seed: context.seed().get(),
            era: Some(era),
            hz: Some(hz),
        })
        .map_err(RacingWorkloadError::Simulation)?;
        let evidence = RacingRunEvidenceV1::from_race_output(&output)?;

        Ok(WorkloadExecution { output, evidence })
    }
}

/// Executes one complete deterministic Racing request.
pub fn run_race(request: RunRaceRequest) -> Result<RaceOutput, String> {
    pitgun_solver::run_race(request)
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

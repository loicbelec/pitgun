//! Transitional public boundary for the Racing reference workload.
//!
//! New consumers should depend directly on `pitgun-racing-simulator`. This
//! module preserves the previous Rust path while downstream code migrates.

pub use pitgun_racing_simulator::evidence::{RacingEvidenceError, RacingRunEvidenceV1};
pub use pitgun_racing_simulator::{
    RaceOutput, RacingWorkload, RacingWorkloadError, RunRaceInput, RunRaceRequest, run_race,
};

#[cfg(test)]
mod tests {
    use super::RacingWorkload;
    use pitgun_runtime::LinkedWorkload;

    #[test]
    fn compatibility_path_keeps_the_published_model_identity() {
        let model = RacingWorkload::v1().model_identity().clone();

        assert_eq!(model.id.to_string(), "pitgun.racing");
        assert_eq!(model.version.to_string(), "1.0.0");
        assert_eq!(
            model.digest.to_string(),
            "sha256:03541bcc24f946d11071e6fb67915ec5d429dce63362d456aba2c3d339a3fe38"
        );
    }
}

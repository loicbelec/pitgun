//! Transitional public boundary for the Racing reference workload.
//!
//! Race orchestration still lives in `pitgun-solver` and is scheduled to move
//! under this crate in #39. Callers use this module so that migration does not
//! change their domain-facing API.

pub use pitgun_solver::evidence::RacingRunEvidenceV1;
pub use pitgun_solver::{RaceOutput, RunRaceInput, RunRaceRequest};

/// Executes one complete deterministic Racing request.
pub fn run_race(request: RunRaceRequest) -> Result<RaceOutput, String> {
    pitgun_solver::run_race(request)
}

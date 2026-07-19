//! Domain-neutral deterministic execution primitives for Pitgun workloads.
//!
//! The runtime owns compatibility-sensitive execution mechanisms. It does not
//! contain domain equations or a universal numerical Solver.

pub mod rng;
mod verification;
mod workload;

pub use verification::{
    LoadedRunBundle, RunBundleArtifactBytes, RunBundleVerificationError, ScenarioBinding,
    VerifiedRunBundle, verify_loaded_run_bundle, verify_run_bundle_artifacts,
};
pub use workload::{
    ExecutedWorkload, ExecutionContext, LinkedExecutionResult, LinkedWorkload, LinkedWorkloadError,
    WorkloadEvidence, WorkloadExecution, execute_linked,
};

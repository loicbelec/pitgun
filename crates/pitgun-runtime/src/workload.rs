//! Compile-time integration boundary for deterministic domain workloads.

use std::error::Error;
use std::fmt;

use pitgun_contract::{
    ArtifactIdentity, CanonicalJsonError, DeterministicRunContractV1, Digest, Seed,
    canonical_json_digest,
};
use serde::Serialize;

/// Read-only deterministic context supplied to one linked workload execution.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionContext<'a> {
    contract: &'a DeterministicRunContractV1,
}

impl<'a> ExecutionContext<'a> {
    /// Returns the complete logical contract bound to this execution.
    #[must_use]
    pub const fn contract(&self) -> &'a DeterministicRunContractV1 {
        self.contract
    }

    /// Returns the root seed declared by the logical contract.
    #[must_use]
    pub const fn seed(&self) -> Seed {
        self.contract.random.seed
    }
}

/// Canonical evidence produced by a domain workload.
///
/// The runtime deliberately knows only the two domain-neutral digests recorded
/// in an execution receipt. Evidence schemas and comparison semantics remain
/// owned by the domain.
pub trait WorkloadEvidence {
    /// Calculates the digest of the complete canonical domain output.
    fn output_digest(&self) -> Result<Digest, CanonicalJsonError>;

    /// Calculates the digest of the canonical telemetry summary.
    fn telemetry_summary_digest(&self) -> Result<Digest, CanonicalJsonError>;
}

/// Output and canonical evidence returned by one domain workload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExecution<Output, Evidence> {
    /// Domain output consumed by applications and persistence adapters.
    pub output: Output,
    /// Domain evidence used by the runtime to bind canonical digests.
    pub evidence: Evidence,
}

/// A Rust workload linked into the current native binary or WASM artifact.
///
/// This is not a numerical Solver abstraction and it is not a dynamic plugin
/// ABI. Domain crates decide how their Solver and Simulator implement it.
pub trait LinkedWorkload {
    /// Canonically serializable input bound by the run contract.
    type Input: Serialize;
    /// Domain output returned after deterministic execution.
    type Output;
    /// Canonical evidence projected from the domain output.
    type Evidence: WorkloadEvidence;
    /// Domain-specific execution failure.
    type Error;

    /// Returns the exact model identity implemented by this adapter.
    fn model_identity(&self) -> &ArtifactIdentity;

    /// Executes the workload with a read-only deterministic context.
    fn execute(
        &self,
        context: &ExecutionContext<'_>,
        input: Self::Input,
    ) -> Result<WorkloadExecution<Self::Output, Self::Evidence>, Self::Error>;
}

/// Logical result of a workload whose contract bindings and evidence digests
/// were calculated by the runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutedWorkload<Output, Evidence> {
    /// Logical identity calculated from the complete run contract.
    pub run_id: Digest,
    /// Digest of the canonical domain output.
    pub output_digest: Digest,
    /// Digest of the canonical telemetry summary.
    pub telemetry_summary_digest: Digest,
    /// Domain output returned by the linked workload.
    pub output: Output,
    /// Domain evidence returned by the linked workload.
    pub evidence: Evidence,
}

/// Failure before, during, or immediately after linked workload execution.
#[derive(Debug)]
pub enum LinkedWorkloadError<WorkloadError> {
    /// The linked adapter does not implement the model required by the contract.
    ModelMismatch {
        /// Model required by the deterministic contract.
        expected: Box<ArtifactIdentity>,
        /// Model implemented by the linked workload adapter.
        actual: Box<ArtifactIdentity>,
    },
    /// Canonical input bytes do not match the digest committed by the contract.
    InputDigestMismatch {
        /// Digest required by the deterministic contract.
        expected: Digest,
        /// Digest calculated from the supplied workload input.
        actual: Digest,
    },
    /// Canonical serialization or run-identity calculation failed.
    Canonicalization(CanonicalJsonError),
    /// The domain workload rejected or failed its execution.
    Workload(WorkloadError),
}

impl<WorkloadError> fmt::Display for LinkedWorkloadError<WorkloadError>
where
    WorkloadError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelMismatch { expected, actual } => write!(
                formatter,
                "linked workload model {}@{} ({}) does not match contract model {}@{} ({})",
                actual.id,
                actual.version,
                actual.digest,
                expected.id,
                expected.version,
                expected.digest
            ),
            Self::InputDigestMismatch { expected, actual } => write!(
                formatter,
                "linked workload input digest mismatch: expected {expected}, got {actual}"
            ),
            Self::Canonicalization(error) => {
                write!(
                    formatter,
                    "cannot calculate canonical workload identity or evidence: {error}"
                )
            }
            Self::Workload(error) => write!(formatter, "linked workload failed: {error}"),
        }
    }
}

impl<WorkloadError> Error for LinkedWorkloadError<WorkloadError>
where
    WorkloadError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonicalization(error) => Some(error),
            Self::Workload(error) => Some(error),
            _ => None,
        }
    }
}

impl<WorkloadError> From<CanonicalJsonError> for LinkedWorkloadError<WorkloadError> {
    fn from(error: CanonicalJsonError) -> Self {
        Self::Canonicalization(error)
    }
}

/// Result returned by [`execute_linked`] for one concrete workload type.
pub type LinkedExecutionResult<Workload> = Result<
    ExecutedWorkload<<Workload as LinkedWorkload>::Output, <Workload as LinkedWorkload>::Evidence>,
    LinkedWorkloadError<<Workload as LinkedWorkload>::Error>,
>;

/// Validates contract bindings and executes one statically linked workload.
pub fn execute_linked<Workload>(
    workload: &Workload,
    contract: &DeterministicRunContractV1,
    input: Workload::Input,
) -> LinkedExecutionResult<Workload>
where
    Workload: LinkedWorkload,
{
    if workload.model_identity() != &contract.model {
        return Err(LinkedWorkloadError::ModelMismatch {
            expected: Box::new(contract.model.clone()),
            actual: Box::new(workload.model_identity().clone()),
        });
    }

    let input_digest = canonical_json_digest(&input)?;
    if input_digest != contract.input.digest {
        return Err(LinkedWorkloadError::InputDigestMismatch {
            expected: contract.input.digest,
            actual: input_digest,
        });
    }

    let run_id = contract.run_id()?;
    let execution = workload
        .execute(&ExecutionContext { contract }, input)
        .map_err(LinkedWorkloadError::Workload)?;
    let output_digest = execution.evidence.output_digest()?;
    let telemetry_summary_digest = execution.evidence.telemetry_summary_digest()?;

    Ok(ExecutedWorkload {
        run_id,
        output_digest,
        telemetry_summary_digest,
        output: execution.output,
        evidence: execution.evidence,
    })
}

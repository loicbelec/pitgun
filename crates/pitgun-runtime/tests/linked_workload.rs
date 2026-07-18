use std::convert::Infallible;

use pitgun_contract::{
    ArtifactIdentity, CanonicalJsonError, ContractVersion, DeterministicRunContractV1, Digest,
    EventOrderingV1, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
    RandomAlgorithm, RandomContractV1, RuntimeProfile, ScenarioIdentity, Seed, StreamDerivation,
    TelemetrySummaryV1, canonical_json_digest,
};
use pitgun_runtime::{
    ExecutionContext, LinkedWorkload, LinkedWorkloadError, WorkloadEvidence, WorkloadExecution,
    execute_linked,
};
use serde::Serialize;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct EchoInput {
    value: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct EchoOutput {
    value: u64,
    seed: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EchoEvidence {
    output: EchoOutput,
    telemetry_summary: TelemetrySummaryV1,
}

impl WorkloadEvidence for EchoEvidence {
    fn output_digest(&self) -> Result<Digest, CanonicalJsonError> {
        canonical_json_digest(&self.output)
    }

    fn telemetry_summary_digest(&self) -> Result<Digest, CanonicalJsonError> {
        canonical_json_digest(&self.telemetry_summary)
    }
}

struct EchoWorkload {
    model: ArtifactIdentity,
}

impl LinkedWorkload for EchoWorkload {
    type Input = EchoInput;
    type Output = EchoOutput;
    type Evidence = EchoEvidence;
    type Error = Infallible;

    fn model_identity(&self) -> &ArtifactIdentity {
        &self.model
    }

    fn execute(
        &self,
        context: &ExecutionContext<'_>,
        input: Self::Input,
    ) -> Result<WorkloadExecution<Self::Output, Self::Evidence>, Self::Error> {
        let output = EchoOutput {
            value: input.value,
            seed: context.seed().to_string(),
        };
        let evidence = EchoEvidence {
            output: output.clone(),
            telemetry_summary: TelemetrySummaryV1::from_ordered_frames(0, [], 0)
                .expect("empty telemetry summary"),
        };
        Ok(WorkloadExecution { output, evidence })
    }
}

fn artifact(id: &str, digest: &str) -> ArtifactIdentity {
    ArtifactIdentity {
        id: id.parse().expect("artifact id"),
        version: "1.0.0".parse().expect("artifact version"),
        digest: Digest::from_bytes(digest.as_bytes()),
    }
}

fn contract(input: &EchoInput) -> DeterministicRunContractV1 {
    DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: "example.echo".parse().expect("scenario id"),
            version: "1.0.0".parse().expect("scenario version"),
        },
        model: artifact("pitgun.example.echo", "echo-model"),
        data_pack: artifact("pitgun.example.empty-data", "empty-data"),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(42),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 1, 1).expect("logical clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: canonical_json_digest(input).expect("input digest"),
        },
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn linked_workload_binds_model_input_context_and_evidence() {
    let input = EchoInput { value: 7 };
    let contract = contract(&input);
    let workload = EchoWorkload {
        model: contract.model.clone(),
    };

    let executed = execute_linked(&workload, &contract, input).expect("linked execution");

    assert_eq!(executed.run_id, contract.run_id().expect("run id"));
    assert_eq!(executed.output.value, 7);
    assert_eq!(executed.output.seed, "42");
    assert_eq!(
        executed.output_digest,
        canonical_json_digest(&executed.evidence.output).expect("output digest")
    );
    assert_eq!(
        executed.telemetry_summary_digest,
        canonical_json_digest(&executed.evidence.telemetry_summary)
            .expect("telemetry summary digest")
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn linked_workload_rejects_a_different_model_before_execution() {
    let input = EchoInput { value: 7 };
    let contract = contract(&input);
    let workload = EchoWorkload {
        model: artifact("pitgun.example.other", "other-model"),
    };

    assert!(matches!(
        execute_linked(&workload, &contract, input),
        Err(LinkedWorkloadError::ModelMismatch { .. })
    ));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn linked_workload_rejects_input_not_bound_by_the_contract() {
    let contracted_input = EchoInput { value: 7 };
    let contract = contract(&contracted_input);
    let workload = EchoWorkload {
        model: contract.model.clone(),
    };

    assert!(matches!(
        execute_linked(&workload, &contract, EchoInput { value: 8 }),
        Err(LinkedWorkloadError::InputDigestMismatch { .. })
    ));
}

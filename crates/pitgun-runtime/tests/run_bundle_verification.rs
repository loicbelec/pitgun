use pitgun_contract::{
    ArtifactIdentity, ContractVersion, DeterministicRunContractV1, Digest, EventOrderingV1,
    ExecutionId, ExecutionReceiptV1, InputCanonicalization, InputIdentity, InputMediaType,
    LogicalClockV1, RandomAlgorithm, RandomContractV1, RunBundleArtifactV1,
    RunBundleCanonicalArtifactsV1, RunBundleExecutionArtifactsV1, RunBundleManifestV1,
    RunBundleManifestVersion, RunBundleMediaType, RunBundleReceiptV1, RunBundleReceiptVersion,
    RunBundleTelemetryRecordV1, RuntimeIdentity, RuntimeProfile, ScenarioIdentity, Seed,
    StreamDerivation, TelemetrySummaryV1, canonical_json_bytes, canonical_json_digest,
};
use pitgun_runtime::{
    LoadedRunBundle, RunBundleArtifactBytes, RunBundleVerificationError, ScenarioBinding,
    verify_loaded_run_bundle,
};
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[derive(Clone)]
struct Fixture {
    scenario_identity: ScenarioIdentity,
    model: ArtifactIdentity,
    data_pack: ArtifactIdentity,
    input_digest: Digest,
    manifest: RunBundleManifestV1,
    contract: DeterministicRunContractV1,
    receipt: RunBundleReceiptV1,
    declared_summary: TelemetrySummaryV1,
    records: Vec<RunBundleTelemetryRecordV1>,
    scenario: Vec<u8>,
    contract_bytes: Vec<u8>,
    output: Vec<u8>,
    telemetry: Vec<u8>,
    telemetry_summary: Vec<u8>,
    metrics: Vec<u8>,
    receipt_bytes: Vec<u8>,
}

fn artifact_identity(id: &str, label: &str) -> ArtifactIdentity {
    ArtifactIdentity {
        id: id.parse().expect("artifact id"),
        version: "1.0.0".parse().expect("artifact version"),
        digest: Digest::from_bytes(label.as_bytes()),
    }
}

fn artifact(path: &str, media_type: RunBundleMediaType, bytes: &[u8]) -> RunBundleArtifactV1 {
    RunBundleArtifactV1 {
        path: path.to_owned(),
        media_type,
        digest: Digest::from_bytes(bytes),
    }
}

fn fixture() -> Fixture {
    let scenario_identity = ScenarioIdentity {
        id: "example.loaded-verification".parse().expect("scenario id"),
        version: "1.0.0".parse().expect("scenario version"),
    };
    let model = artifact_identity("pitgun.example.model", "model");
    let data_pack = artifact_identity("pitgun.example.data", "data");
    let request = json!({"value": 7});
    let input_digest = canonical_json_digest(&request).expect("input digest");
    let contract = DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: scenario_identity.clone(),
        model: model.clone(),
        data_pack: data_pack.clone(),
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
            digest: input_digest,
        },
    };

    let scenario = canonical_json_bytes(&json!({
        "schema_version": "pitgun.example-scenario/v1",
        "scenario": scenario_identity,
        "model": model,
        "data_pack": data_pack,
        "request": request,
    }))
    .expect("scenario bytes");
    let contract_bytes = canonical_json_bytes(&contract).expect("contract bytes");
    let output = canonical_json_bytes(&json!({
        "schema_version": "pitgun.example-output/v1",
        "value": 7,
    }))
    .expect("output bytes");
    let records = Vec::new();
    let telemetry = Vec::new();
    let declared_summary =
        TelemetrySummaryV1::from_ordered_frames(0, [], 0).expect("empty summary");
    let telemetry_summary =
        canonical_json_bytes(&declared_summary).expect("telemetry summary bytes");
    let metrics = canonical_json_bytes(&json!({
        "schema_version": "pitgun.derived-metrics/v1",
        "metrics": [],
    }))
    .expect("metrics bytes");

    let runtime = RuntimeIdentity {
        engine: "pitgun-test".parse().expect("runtime engine"),
        engine_version: "1.0.0".parse().expect("runtime version"),
        target: "wasm32-unknown-unknown".parse().expect("runtime target"),
        artifact_digest: Digest::from_bytes(b"runtime"),
    };
    let execution_id: ExecutionId = "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
        .parse()
        .expect("execution id");
    let receipt = RunBundleReceiptV1 {
        schema_version: RunBundleReceiptVersion::V1,
        receipt: ExecutionReceiptV1::for_contract(
            &contract,
            execution_id,
            runtime,
            Digest::from_bytes(&output),
            Digest::from_bytes(&telemetry_summary),
        )
        .expect("execution receipt"),
    };
    let receipt_bytes = canonical_json_bytes(&receipt).expect("receipt bytes");

    let manifest = RunBundleManifestV1 {
        schema_version: RunBundleManifestVersion::V1,
        run_id: contract.run_id().expect("run id"),
        canonical_artifacts: RunBundleCanonicalArtifactsV1 {
            scenario: artifact(
                "scenario.json",
                RunBundleMediaType::ApplicationJson,
                &scenario,
            ),
            contract: artifact(
                "contract.json",
                RunBundleMediaType::ApplicationJson,
                &contract_bytes,
            ),
            output: artifact("output.json", RunBundleMediaType::ApplicationJson, &output),
            telemetry: artifact(
                "telemetry.jsonl",
                RunBundleMediaType::ApplicationNdjson,
                &telemetry,
            ),
            telemetry_summary: artifact(
                "telemetry-summary.json",
                RunBundleMediaType::ApplicationJson,
                &telemetry_summary,
            ),
            metrics: artifact(
                "metrics.json",
                RunBundleMediaType::ApplicationJson,
                &metrics,
            ),
        },
        execution_artifacts: RunBundleExecutionArtifactsV1 {
            receipt: artifact(
                "receipt.json",
                RunBundleMediaType::ApplicationJson,
                &receipt_bytes,
            ),
        },
    };

    Fixture {
        scenario_identity: contract.scenario.clone(),
        model: contract.model.clone(),
        data_pack: contract.data_pack.clone(),
        input_digest,
        manifest,
        contract,
        receipt,
        declared_summary,
        records,
        scenario,
        contract_bytes,
        output,
        telemetry,
        telemetry_summary,
        metrics,
        receipt_bytes,
    }
}

fn verify(
    fixture: &Fixture,
) -> Result<pitgun_runtime::VerifiedRunBundle, RunBundleVerificationError> {
    verify_loaded_run_bundle(LoadedRunBundle {
        manifest: &fixture.manifest,
        contract: &fixture.contract,
        receipt: &fixture.receipt,
        scenario: ScenarioBinding {
            scenario: &fixture.scenario_identity,
            model: &fixture.model,
            data_pack: &fixture.data_pack,
            input_digest: fixture.input_digest,
        },
        artifacts: RunBundleArtifactBytes {
            scenario: &fixture.scenario,
            contract: &fixture.contract_bytes,
            output: &fixture.output,
            telemetry: &fixture.telemetry,
            telemetry_summary: &fixture.telemetry_summary,
            metrics: &fixture.metrics,
            receipt: &fixture.receipt_bytes,
        },
        declared_telemetry_summary: &fixture.declared_summary,
        telemetry_records: &fixture.records,
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn loaded_bundle_verification_recalculates_domain_neutral_evidence() {
    let fixture = fixture();

    let verified = verify(&fixture).expect("verified loaded bundle");

    assert_eq!(verified.run_id, fixture.manifest.run_id);
    assert_eq!(verified.telemetry_summary, fixture.declared_summary);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn loaded_bundle_verification_rejects_mutated_artifact_bytes() {
    let mut fixture = fixture();
    fixture.output.push(b' ');

    assert!(matches!(
        verify(&fixture),
        Err(RunBundleVerificationError::ArtifactDigestMismatch {
            artifact: "output.json",
            ..
        })
    ));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn loaded_bundle_verification_rejects_unbound_scenario_input() {
    let mut fixture = fixture();
    fixture.input_digest = Digest::from_bytes(b"different input");

    assert!(matches!(
        verify(&fixture),
        Err(RunBundleVerificationError::InputDigestMismatch { .. })
    ));
}

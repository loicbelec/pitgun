use std::fmt::Debug;

use pitgun_contract::{
    ArtifactIdentity, AuthorizationSignatureAlgorithm, AuthorizationValidityV1, ContractVersion,
    DeterministicRunContractV1, Digest, EventOrderingV1, Identifier, InputCanonicalization,
    InputIdentity, InputMediaType, LogicalClockV1, RandomAlgorithm, RandomContractV1,
    RunAuthorizationV1, RunAuthorizationVersion, RuntimeIdentity, RuntimeProfile, ScenarioIdentity,
    Seed, SemanticVersion, SignedRunAuthorizationV1, StreamDerivation, canonical_json_bytes,
    canonical_json_digest, canonicalize_json_str,
};
use pitgun_racing_simulator::{
    CurvatureAeroResponse, TuningResponseV1, racing_model_v3_candidate_identity,
    racing_model_v3_thermal_candidate_identity, run_race_with_catalog_and_model_response,
    run_race_with_catalog_and_v3_candidate,
};
use pitgun_solver::evidence::{
    RacingExecutionResolutionVersion, RacingHostedExecutionRequestV1,
    RacingHostedExecutionRequestVersion, RacingRunEvidenceV1, RacingVerificationSubmissionV1,
};
use pitgun_solver::{
    RaceOutput, RacingCatalogFileV1, RacingCatalogSnapshot, RunRaceInput, RunRaceRequest,
    execute_authorized_race, execute_authorized_race_json, racing_model_v1_identity,
    racing_model_v2_identity, run_race_json,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const INPUT: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v1.input.json");
const EXPECTED: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v1.expected.json");
const CONTRACT: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v1.contract.json");
const EXPECTED_OUTPUT: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v1.output.json");
const EXPECTED_TELEMETRY_SUMMARY: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v1.telemetry-summary.json");
const EXPECTED_DIGESTS: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v1.digests.json");
const EXPECTED_HOSTED_WASM_DIGESTS: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_hosted_wasm_v1.digests.json");
const INPUT_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.input.json");
const EXPECTED_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.expected.json");
const CONTRACT_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.contract.json");
const EXPECTED_OUTPUT_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.output.json");
const EXPECTED_TELEMETRY_SUMMARY_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.telemetry-summary.json");
const EXPECTED_DIGESTS_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.digests.json");
const EXPECTED_RECEIPT_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.receipt.json");
const EXPECTED_HOSTED_WASM_DIGESTS_V2: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_hosted_wasm_v2.digests.json");
const MODEL_PARAMETERS_V2: &str = include_str!(
    "../../../catalogs/racing/v1.4.0/simulation/model-parameters/v2-compatibility.json"
);
const MODEL_IDENTITY: &str = "pitgun.racing:model:1.0.0:conformance-vector";
const DATA_PACK_IDENTITY: &str = "pitgun.racing.2026:data-pack:1.0.0:conformance-vector";

#[cfg(target_arch = "wasm32")]
const TARGET: &str = "wasm32-unknown-unknown";
#[cfg(not(target_arch = "wasm32"))]
const TARGET: &str = "native-test-target";

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct GoldenSummary {
    total_time_ms: u64,
    player_lap_times_ms: Vec<u64>,
    standings: Vec<GoldenStanding>,
    telemetry: GoldenTelemetry,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn v3_candidate_contact_patch_is_deterministic_across_supported_runtimes() {
    let request: RunRaceRequest = serde_json::from_str(INPUT_V2).expect("V2 input fixture");
    let catalog = RacingCatalogSnapshot::embedded_model_v2().expect("embedded model V2 catalog");
    let first = run_race_with_catalog_and_v3_candidate(
        request.clone(),
        &catalog,
        &TuningResponseV1::default(),
    )
    .expect("first V3 candidate run");
    let second =
        run_race_with_catalog_and_v3_candidate(request, &catalog, &TuningResponseV1::default())
            .expect("second V3 candidate run");

    assert_eq!(
        canonical_json_bytes(&first).expect("first canonical candidate output"),
        canonical_json_bytes(&second).expect("second canonical candidate output"),
    );
    assert_eq!(
        racing_model_v3_candidate_identity().version.to_string(),
        "0.9.0"
    );
    assert_eq!(
        first.total_time_ms, 95_977,
        "V3 candidate output changed without a new candidate identity"
    );
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct GoldenStanding {
    competitor_id: String,
    position: u32,
    total_time_ms: u64,
    best_lap_ms: u64,
    laps_completed: u16,
    gap_to_leader_ms: u64,
    status: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct GoldenTelemetry {
    batch_count: usize,
    frame_count: usize,
    first_timestamp_us: i64,
    last_timestamp_us: i64,
    first_sequence: u64,
    last_sequence: u64,
    samples_per_frame: usize,
    parameter_ids: Vec<u16>,
    first_lap_number: Option<u16>,
    last_lap_number: Option<u16>,
    source_id: String,
    sampling_hz: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct GoldenDigests {
    run_id: Digest,
    output_digest: Digest,
    telemetry_summary_digest: Digest,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct HostedWasmGoldenDigests {
    run_id: Digest,
    output_digest: Digest,
    telemetry_summary_digest: Digest,
    wasm_artifact_digest: Digest,
}

fn parameter_backed_v2_catalog() -> RacingCatalogSnapshot {
    let historical = RacingCatalogSnapshot::embedded_model_v2().expect("model V2 catalog");
    let mut bundle = historical.to_bundle().expect("catalog bundle");
    let parameter_digest = Digest::from_bytes(MODEL_PARAMETERS_V2.as_bytes());

    let mut index: serde_json::Value =
        serde_json::from_str(&bundle.simulation_index).expect("simulation index");
    let resources = index["resources"]
        .as_array_mut()
        .expect("simulation resources");
    resources.push(serde_json::json!({
        "id": "pitgun.racing.model-parameters.v2-compatibility",
        "path": "simulation/model-parameters/v2-compatibility.json",
        "media_type": "application/json",
        "digest": parameter_digest,
    }));
    resources.sort_by(|left, right| {
        left["id"]
            .as_str()
            .expect("resource id")
            .cmp(right["id"].as_str().expect("resource id"))
    });
    let simulation_pack_digest = canonical_json_digest(&index).expect("Simulation Pack digest");
    bundle.simulation_index = serde_json::to_string(&index).expect("simulation index JSON");
    bundle.resources.push(RacingCatalogFileV1 {
        path: "simulation/model-parameters/v2-compatibility.json".to_string(),
        contents: MODEL_PARAMETERS_V2.to_string(),
    });

    let mut manifest: serde_json::Value =
        serde_json::from_str(&bundle.manifest).expect("catalog manifest");
    manifest["catalog"]["version"] = serde_json::json!("1.4.0");
    manifest["simulation_pack"]["identity"]["version"] = serde_json::json!("1.4.0");
    manifest["simulation_pack"]["identity"]["digest"] =
        serde_json::to_value(simulation_pack_digest).expect("pack identity digest");
    manifest["simulation_pack"]["index"]["digest"] =
        serde_json::to_value(simulation_pack_digest).expect("pack index digest");
    let manifest_digest = canonical_json_digest(&manifest).expect("catalog manifest digest");
    bundle.manifest = serde_json::to_string(&manifest).expect("catalog manifest JSON");
    bundle.release_identity = serde_json::to_string(&serde_json::json!({
        "schema_version": "pitgun.catalog-release-identity/v1",
        "id": "pitgun.racing",
        "version": "1.4.0",
        "manifest_digest": manifest_digest,
    }))
    .expect("release identity JSON");

    let bundle_json = serde_json::to_string(&bundle).expect("browser catalog bundle");
    RacingCatalogSnapshot::from_bundle_json(&bundle_json).expect("parameter-backed browser catalog")
}

fn summarize(output: RaceOutput) -> GoldenSummary {
    let frames = output
        .player_batches
        .iter()
        .flat_map(|batch| batch.frames.iter())
        .collect::<Vec<_>>();
    let first = frames.first().expect("golden run must emit telemetry");
    let last = frames.last().expect("golden run must emit telemetry");

    GoldenSummary {
        total_time_ms: output.total_time_ms,
        player_lap_times_ms: output.player_lap_times_ms,
        standings: output
            .standings
            .into_iter()
            .map(|standing| GoldenStanding {
                competitor_id: standing.competitor_id,
                position: standing.position,
                total_time_ms: standing.total_time_ms,
                best_lap_ms: standing.best_lap_ms,
                laps_completed: standing.laps_completed,
                gap_to_leader_ms: standing.gap_to_leader_ms,
                status: serde_json::to_value(standing.status)
                    .expect("standing status must serialize")
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .expect("standing status must have a type")
                    .to_string(),
            })
            .collect(),
        telemetry: GoldenTelemetry {
            batch_count: output.player_batches.len(),
            frame_count: frames.len(),
            first_timestamp_us: first.timestamp_us,
            last_timestamp_us: last.timestamp_us,
            first_sequence: first.sequence,
            last_sequence: last.sequence,
            samples_per_frame: first.samples.len(),
            parameter_ids: first
                .samples
                .iter()
                .map(|sample| sample.parameter_id)
                .collect(),
            first_lap_number: first.cycle_index,
            last_lap_number: last.cycle_index,
            source_id: first.source_id.clone(),
            sampling_hz: first
                .metadata
                .get("sampling_hz")
                .expect("sampling_hz metadata")
                .clone(),
        },
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v1_matches_the_versioned_golden_summary() {
    let output = run_golden_race();
    let actual = summarize(output);
    let expected: GoldenSummary =
        serde_json::from_str(EXPECTED).expect("golden summary fixture must be valid");

    assert_eq!(
        actual,
        expected,
        "Racing golden run changed. Update the model or contract version before accepting a new fixture.\nActual summary:\n{}",
        serde_json::to_string_pretty(&actual).expect("golden summary must serialize")
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v1_matches_published_canonical_artifacts_and_digests() {
    let output = run_golden_race();
    let evidence = RacingRunEvidenceV1::from_race_output(&output).expect("Racing evidence");
    let contract: DeterministicRunContractV1 =
        serde_json::from_str(CONTRACT).expect("Racing deterministic contract");

    assert_artifact_eq("Racing output", &evidence.output, EXPECTED_OUTPUT);
    assert_artifact_eq(
        "telemetry summary",
        &evidence.telemetry_summary,
        EXPECTED_TELEMETRY_SUMMARY,
    );

    let canonical_input = canonicalize_json_str(INPUT).expect("canonical Racing input");
    let input_digest = Digest::from_bytes(&canonical_input);
    assert_eq!(
        contract.input.digest, input_digest,
        "contract input.digest must bind the canonical Racing input"
    );
    assert_eq!(
        contract.model.digest,
        Digest::from_bytes(MODEL_IDENTITY.as_bytes()),
        "contract model.digest must bind the published conformance identity"
    );
    assert_eq!(
        contract.data_pack.digest,
        Digest::from_bytes(DATA_PACK_IDENTITY.as_bytes()),
        "contract data_pack.digest must bind the published conformance identity"
    );

    let execution_id = "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
        .parse()
        .expect("execution id");
    let receipt = evidence
        .execution_receipt(
            &contract,
            execution_id,
            RuntimeIdentity {
                engine: Identifier::new("pitgun-rust").expect("engine id"),
                engine_version: SemanticVersion::new("0.1.0").expect("engine version"),
                target: Identifier::new(TARGET).expect("target id"),
                artifact_digest: Digest::from_bytes(TARGET.as_bytes()),
            },
        )
        .expect("execution receipt");
    let actual = GoldenDigests {
        run_id: receipt.run_id,
        output_digest: receipt.output_digest,
        telemetry_summary_digest: receipt.telemetry_summary_digest,
    };
    let expected: GoldenDigests =
        serde_json::from_str(EXPECTED_DIGESTS).expect("published digest vectors");

    assert_eq!(
        actual,
        expected,
        "Racing deterministic digests changed. Compare the canonical output and telemetry summary artifacts before updating this vector.\nActual:\n{}",
        serde_json::to_string_pretty(&actual).expect("actual digests must serialize")
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v2_matches_the_versioned_golden_summary() {
    let output = run_golden_race_v2();
    let actual = summarize(output);
    let expected: GoldenSummary =
        serde_json::from_str(EXPECTED_V2).expect("V2 golden summary fixture must be valid");

    assert_eq!(
        actual,
        expected,
        "Racing V2 golden run changed. Publish a new model identity before accepting a new fixture.\nActual summary:\n{}",
        serde_json::to_string_pretty(&actual).expect("golden summary must serialize")
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v2_matches_published_canonical_artifacts_and_digests() {
    let output = run_golden_race_v2();
    let evidence = RacingRunEvidenceV1::from_race_output(&output).expect("Racing V2 evidence");
    let contract: DeterministicRunContractV1 =
        serde_json::from_str(CONTRACT_V2).expect("Racing V2 deterministic contract");

    assert_artifact_eq("Racing V2 output", &evidence.output, EXPECTED_OUTPUT_V2);
    assert_artifact_eq(
        "V2 telemetry summary",
        &evidence.telemetry_summary,
        EXPECTED_TELEMETRY_SUMMARY_V2,
    );
    assert_eq!(contract.model, racing_model_v2_identity());
    let catalog = RacingCatalogSnapshot::embedded_model_v2().expect("model V2 catalog");
    assert_eq!(
        contract.data_pack,
        catalog.manifest().simulation_pack.identity
    );

    let canonical_input = canonicalize_json_str(INPUT_V2).expect("canonical Racing V2 input");
    assert_eq!(contract.input.digest, Digest::from_bytes(&canonical_input));
    let execution_id = "018f3b78-7e9a-7d20-a5e1-4ed92f02a592"
        .parse()
        .expect("V2 execution id");
    let receipt = evidence
        .execution_receipt(
            &contract,
            execution_id,
            RuntimeIdentity {
                engine: Identifier::new("pitgun-rust").expect("engine id"),
                engine_version: SemanticVersion::new("0.1.0").expect("engine version"),
                target: Identifier::new("portable-golden-target").expect("target id"),
                artifact_digest: Digest::from_bytes(b"portable-golden-artifact-v2"),
            },
        )
        .expect("V2 execution receipt");
    let actual_receipt = serde_json::to_value(&receipt).expect("V2 receipt value");
    assert_artifact_eq("Racing V2 receipt", &actual_receipt, EXPECTED_RECEIPT_V2);
    let actual = GoldenDigests {
        run_id: receipt.run_id,
        output_digest: receipt.output_digest,
        telemetry_summary_digest: receipt.telemetry_summary_digest,
    };
    let expected: GoldenDigests =
        serde_json::from_str(EXPECTED_DIGESTS_V2).expect("published V2 digest vectors");
    assert_eq!(
        actual,
        expected,
        "Racing V2 deterministic digests changed.\nActual:\n{}",
        serde_json::to_string_pretty(&actual).expect("actual digests must serialize")
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_run_v1_digests_reject_semantic_mutations() {
    let output = run_golden_race();
    let evidence = RacingRunEvidenceV1::from_race_output(&output).expect("Racing evidence");
    let contract: DeterministicRunContractV1 =
        serde_json::from_str(CONTRACT).expect("Racing deterministic contract");

    let original_run_id = contract.run_id().expect("run id");
    let mut changed_contract = contract;
    changed_contract.random.seed = Seed::new(8);
    assert_ne!(
        changed_contract.run_id().expect("changed run id"),
        original_run_id,
        "an input contract mutation must change run_id"
    );

    let original_output_digest = evidence.output_digest().expect("output digest");
    let mut changed_output = evidence.output.clone();
    changed_output.total_time_ms += 1;
    assert_ne!(
        canonical_json_digest(&changed_output).expect("changed output digest"),
        original_output_digest,
        "a domain output mutation must change output_digest"
    );

    let original_summary_digest = evidence
        .telemetry_summary_digest()
        .expect("telemetry summary digest");
    let mut changed_summary =
        serde_json::to_value(evidence.telemetry_summary).expect("summary value");
    changed_summary["dropped_frame_count"] = serde_json::json!(1);
    assert_ne!(
        canonical_json_digest(&changed_summary).expect("changed summary digest"),
        original_summary_digest,
        "a telemetry summary mutation must change telemetry_summary_digest"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn hosted_racing_execution_emits_complete_portable_evidence() {
    let document: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/pitgun-cli/scenarios/racing-demo-v1.json"
    ))
    .expect("Racing demo");
    let input: RunRaceInput =
        serde_json::from_value(document["request"].clone()).expect("Racing input");
    let catalog = RacingCatalogSnapshot::embedded().expect("embedded catalog");
    let contract = DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: "racing.race".parse().expect("scenario id"),
            version: "1.0.0".parse().expect("scenario version"),
        },
        model: racing_model_v1_identity(),
        data_pack: catalog.manifest().simulation_pack.identity.clone(),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(42),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 200_000, 1).expect("clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: canonical_json_digest(&input).expect("input digest"),
        },
    };
    let run_id = contract.run_id().expect("run id");
    let request = RacingHostedExecutionRequestV1 {
        schema_version: RacingHostedExecutionRequestVersion::V1,
        signed_authorization: SignedRunAuthorizationV1 {
            authorization: RunAuthorizationV1 {
                authorization_version: RunAuthorizationVersion::V1,
                nonce: Digest::from_bytes(b"wasm-golden-nonce"),
                subject: "career.wasm-golden".parse().expect("subject"),
                audience: "pitgun.verifier".parse().expect("audience"),
                contract,
                run_id,
                policy: ArtifactIdentity {
                    id: "pitgun.racing.tuning".parse().expect("policy id"),
                    version: "1.0.0".parse().expect("policy version"),
                    digest: Digest::from_bytes(include_bytes!(
                        "../../../policies/gametuning.v1.yaml"
                    )),
                },
                signing_key_id: "wasm-golden-v1".parse().expect("key id"),
                validity: AuthorizationValidityV1 {
                    issued_at_ms: 1_722_345_600_000,
                    expires_at_ms: 1_722_345_900_000,
                    late_submission_grace_ms: 900_000,
                },
            },
            algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
            signature: "fixture-only-signature".to_string(),
        },
        input,
        execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
            .parse()
            .expect("execution id"),
        wasm_artifact_digest: Digest::from_bytes(b"exact-golden-wasm-module"),
    };
    let response = execute_authorized_race_json(
        serde_json::to_string(&request).expect("hosted execution request"),
    );
    let submission: RacingVerificationSubmissionV1 = serde_json::from_str(&response)
        .unwrap_or_else(|error| panic!("invalid hosted evidence: {error}: {response}"));

    let actual = HostedWasmGoldenDigests {
        run_id: submission.receipt.receipt.run_id,
        output_digest: submission.receipt.receipt.output_digest,
        telemetry_summary_digest: submission.receipt.receipt.telemetry_summary_digest,
        wasm_artifact_digest: submission.receipt.receipt.runtime.artifact_digest,
    };
    let expected: HostedWasmGoldenDigests =
        serde_json::from_str(EXPECTED_HOSTED_WASM_DIGESTS).expect("hosted WASM digest fixture");
    assert_eq!(actual, expected, "hosted WASM evidence changed");
    assert_eq!(submission.receipt.receipt.run_id, run_id);
    assert_eq!(
        submission.receipt.receipt.runtime.engine.to_string(),
        "pitgun-wasm"
    );
    assert_eq!(
        submission.receipt.receipt.runtime.target.to_string(),
        "wasm32-unknown-unknown"
    );
    assert_eq!(
        submission.receipt.receipt.runtime.artifact_digest,
        request.wasm_artifact_digest
    );
    assert_eq!(
        submission.receipt.receipt.output_digest,
        canonical_json_digest(&submission.output).expect("output digest")
    );
    assert_eq!(
        submission.receipt.receipt.telemetry_summary_digest,
        canonical_json_digest(&submission.telemetry_summary).expect("telemetry digest")
    );
    let round_trip = serde_json::to_string(&submission).expect("submission JSON");
    serde_json::from_str::<RacingVerificationSubmissionV1>(&round_trip)
        .expect("strict Verifier submission");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn hosted_racing_v2_execution_emits_complete_portable_evidence() {
    let request_fixture: RunRaceRequest =
        serde_json::from_str(INPUT_V2).expect("Racing V2 request");
    let input = request_fixture.input;
    let catalog = RacingCatalogSnapshot::embedded_model_v2().expect("model V2 catalog");
    let contract = DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: "racing.race".parse().expect("scenario id"),
            version: "2.0.0".parse().expect("scenario version"),
        },
        model: racing_model_v2_identity(),
        data_pack: catalog.manifest().simulation_pack.identity.clone(),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(7),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 50_000, 1).expect("clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: canonical_json_digest(&input).expect("input digest"),
        },
    };
    let run_id = contract.run_id().expect("V2 run id");
    let request = RacingHostedExecutionRequestV1 {
        schema_version: RacingHostedExecutionRequestVersion::V1,
        signed_authorization: SignedRunAuthorizationV1 {
            authorization: RunAuthorizationV1 {
                authorization_version: RunAuthorizationVersion::V1,
                nonce: Digest::from_bytes(b"wasm-golden-v2-nonce"),
                subject: "career.wasm-golden-v2".parse().expect("subject"),
                audience: "pitgun.verifier".parse().expect("audience"),
                contract,
                run_id,
                policy: ArtifactIdentity {
                    id: "pitgun.racing.tuning".parse().expect("policy id"),
                    version: "1.0.0".parse().expect("policy version"),
                    digest: Digest::from_bytes(include_bytes!(
                        "../../../policies/gametuning.v1.yaml"
                    )),
                },
                signing_key_id: "wasm-golden-v2".parse().expect("key id"),
                validity: AuthorizationValidityV1 {
                    issued_at_ms: 1_722_345_600_000,
                    expires_at_ms: 1_722_345_900_000,
                    late_submission_grace_ms: 900_000,
                },
            },
            algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
            signature: "fixture-only-signature".to_string(),
        },
        input,
        execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a592"
            .parse()
            .expect("execution id"),
        wasm_artifact_digest: Digest::from_bytes(b"exact-golden-wasm-module-v2"),
    };
    let submission = execute_authorized_race(request.clone(), &catalog)
        .unwrap_or_else(|error| panic!("invalid hosted V2 evidence: {error}"));
    let actual = HostedWasmGoldenDigests {
        run_id: submission.receipt.receipt.run_id,
        output_digest: submission.receipt.receipt.output_digest,
        telemetry_summary_digest: submission.receipt.receipt.telemetry_summary_digest,
        wasm_artifact_digest: submission.receipt.receipt.runtime.artifact_digest,
    };
    let expected: HostedWasmGoldenDigests = serde_json::from_str(EXPECTED_HOSTED_WASM_DIGESTS_V2)
        .expect("hosted WASM V2 digest fixture");
    assert_eq!(
        actual,
        expected,
        "hosted WASM V2 evidence changed. Actual:\n{}",
        serde_json::to_string_pretty(&actual).expect("hosted V2 digests")
    );
    assert_eq!(submission.receipt.receipt.run_id, run_id);
    assert_eq!(
        submission.receipt.receipt.runtime.engine.to_string(),
        "pitgun-wasm"
    );
    assert_eq!(
        submission.receipt.receipt.runtime.target.to_string(),
        "wasm32-unknown-unknown"
    );
    assert_eq!(
        submission.receipt.receipt.runtime.artifact_digest,
        request.wasm_artifact_digest
    );
    assert!(
        submission.execution_resolution.is_none(),
        "historical V2 evidence must retain its exact wire shape"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn parameter_resource_has_identical_native_and_wasm_evidence() {
    let request_fixture: RunRaceRequest =
        serde_json::from_str(INPUT_V2).expect("Racing V2 request");
    let input = request_fixture.input;
    let catalog = parameter_backed_v2_catalog();
    let contract = DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: "racing.race".parse().expect("scenario id"),
            version: "2.0.0".parse().expect("scenario version"),
        },
        model: racing_model_v2_identity(),
        data_pack: catalog.manifest().simulation_pack.identity.clone(),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(7),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 50_000, 1).expect("clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: canonical_json_digest(&input).expect("input digest"),
        },
    };
    let run_id = contract.run_id().expect("parameter-backed run id");
    let submission = execute_authorized_race(
        RacingHostedExecutionRequestV1 {
            schema_version: RacingHostedExecutionRequestVersion::V1,
            signed_authorization: SignedRunAuthorizationV1 {
                authorization: RunAuthorizationV1 {
                    authorization_version: RunAuthorizationVersion::V1,
                    nonce: Digest::from_bytes(b"wasm-golden-v2-parameter-nonce"),
                    subject: "career.wasm-golden-v2-parameters".parse().expect("subject"),
                    audience: "pitgun.verifier".parse().expect("audience"),
                    contract,
                    run_id,
                    policy: ArtifactIdentity {
                        id: "pitgun.racing.tuning".parse().expect("policy id"),
                        version: "1.0.0".parse().expect("policy version"),
                        digest: Digest::from_bytes(include_bytes!(
                            "../../../policies/gametuning.v1.yaml"
                        )),
                    },
                    signing_key_id: "wasm-golden-v2-parameters".parse().expect("key id"),
                    validity: AuthorizationValidityV1 {
                        issued_at_ms: 1_722_345_600_000,
                        expires_at_ms: 1_722_345_900_000,
                        late_submission_grace_ms: 900_000,
                    },
                },
                algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
                signature: "fixture-only-signature".to_string(),
            },
            input,
            execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a593"
                .parse()
                .expect("execution id"),
            wasm_artifact_digest: Digest::from_bytes(b"exact-golden-wasm-module-v2-parameters"),
        },
        &catalog,
    )
    .expect("parameter-backed hosted execution");

    assert_artifact_eq(
        "parameter-backed output",
        &submission.output,
        EXPECTED_OUTPUT_V2,
    );
    assert_artifact_eq(
        "parameter-backed telemetry summary",
        &submission.telemetry_summary,
        EXPECTED_TELEMETRY_SUMMARY_V2,
    );
    let resolution = submission
        .execution_resolution
        .as_ref()
        .expect("parameter-backed execution resolution");
    assert_eq!(resolution.catalog_release, *catalog.release_identity());
    assert_eq!(
        resolution.simulation_pack,
        catalog.manifest().simulation_pack.identity
    );
    assert_eq!(resolution.model, racing_model_v2_identity());
    assert_eq!(
        resolution.model_parameters.as_ref(),
        catalog.model_parameters_identity()
    );
    assert_eq!(
        resolution
            .model_parameters
            .as_ref()
            .expect("parameter identity")
            .digest
            .to_string(),
        "sha256:89c0da5b058cf51b43953d0d31fe2e0f61f3c7038f9149e2fa59ad92c930ef71"
    );
    assert!(resolution.thermal_family_profile.is_none());
    assert_eq!(submission.receipt.receipt.run_id, run_id);
    assert_eq!(
        run_id.to_string(),
        "sha256:453cddb3697c1a60d252b8da2e2277916f46c2c8e7a42dc6eb79daba18f616b7",
        "pin this identity for native/WASM parity"
    );

    let round_trip = serde_json::to_string(&submission).expect("submission JSON");
    let decoded = serde_json::from_str::<RacingVerificationSubmissionV1>(&round_trip)
        .expect("strict parameter-backed submission");
    assert_eq!(
        canonical_json_bytes(&decoded).expect("decoded submission bytes"),
        canonical_json_bytes(&submission).expect("submission bytes")
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn thermal_candidate_has_identical_native_and_wasm_lineage() {
    let request_fixture: RunRaceRequest =
        serde_json::from_str(INPUT_V2).expect("Racing input fixture");
    let input = request_fixture.input;
    let catalog =
        RacingCatalogSnapshot::embedded_model_v3_thermal().expect("Model V3 thermal catalog");
    let model = racing_model_v3_thermal_candidate_identity();
    let contract = DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: "racing.race".parse().expect("scenario id"),
            version: "1.0.0".parse().expect("scenario version"),
        },
        model: model.clone(),
        data_pack: catalog.manifest().simulation_pack.identity.clone(),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(7),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 50_000, 1).expect("clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: canonical_json_digest(&input).expect("input digest"),
        },
    };
    let run_id = contract.run_id().expect("Model V3 thermal run id");
    let submission = execute_authorized_race(
        RacingHostedExecutionRequestV1 {
            schema_version: RacingHostedExecutionRequestVersion::V1,
            signed_authorization: SignedRunAuthorizationV1 {
                authorization: RunAuthorizationV1 {
                    authorization_version: RunAuthorizationVersion::V1,
                    nonce: Digest::from_bytes(b"wasm-golden-v3-thermal-nonce"),
                    subject: "career.wasm-golden-v3-thermal".parse().expect("subject"),
                    audience: "pitgun.verifier".parse().expect("audience"),
                    contract,
                    run_id,
                    policy: ArtifactIdentity {
                        id: "pitgun.racing.tuning".parse().expect("policy id"),
                        version: "1.0.0".parse().expect("policy version"),
                        digest: Digest::from_bytes(include_bytes!(
                            "../../../policies/gametuning.v1.yaml"
                        )),
                    },
                    signing_key_id: "wasm-golden-v3-thermal".parse().expect("key id"),
                    validity: AuthorizationValidityV1 {
                        issued_at_ms: 1_722_345_600_000,
                        expires_at_ms: 1_722_345_900_000,
                        late_submission_grace_ms: 900_000,
                    },
                },
                algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
                signature: "fixture-only-signature".to_string(),
            },
            input,
            execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a594"
                .parse()
                .expect("execution id"),
            wasm_artifact_digest: Digest::from_bytes(b"exact-golden-wasm-module-v3-thermal"),
        },
        &catalog,
    )
    .expect("Model V3 thermal hosted execution");
    let resolution = submission
        .execution_resolution
        .as_ref()
        .expect("Model V3 thermal execution resolution");

    assert_eq!(submission.receipt.receipt.run_id, run_id);
    assert_eq!(
        resolution.schema_version,
        RacingExecutionResolutionVersion::V2
    );
    assert_eq!(resolution.model, model);
    assert!(resolution.model_parameters.is_none());
    assert_eq!(
        resolution.thermal_family_profile.as_ref(),
        catalog.thermal_family_profile_identity()
    );
    let round_trip = serde_json::to_string(&submission).expect("submission JSON");
    let decoded = serde_json::from_str::<RacingVerificationSubmissionV1>(&round_trip)
        .expect("strict Model V3 thermal submission");
    assert_eq!(
        canonical_json_bytes(&decoded).expect("decoded submission bytes"),
        canonical_json_bytes(&submission).expect("submission bytes")
    );
}

fn run_golden_race() -> RaceOutput {
    let response = run_race_json(INPUT.to_string());
    serde_json::from_str(&response)
        .unwrap_or_else(|error| panic!("golden run returned invalid output: {error}: {response}"))
}

fn run_golden_race_v2() -> RaceOutput {
    let request: RunRaceRequest =
        serde_json::from_str(INPUT_V2).expect("Racing V2 request fixture");
    let catalog = RacingCatalogSnapshot::embedded_model_v2().expect("model V2 catalog");
    run_race_with_catalog_and_model_response(
        request,
        &catalog,
        &TuningResponseV1::default(),
        CurvatureAeroResponse::ContinuousV1,
    )
    .expect("Racing V2 golden run")
}

fn assert_artifact_eq<T>(label: &str, actual: &T, expected_json: &str)
where
    T: Debug + DeserializeOwned + PartialEq + Serialize,
{
    let expected: T = serde_json::from_str(expected_json)
        .unwrap_or_else(|error| panic!("invalid expected {label} artifact: {error}"));
    assert_eq!(
        actual,
        &expected,
        "{label} changed before digest comparison.\nActual:\n{}",
        serde_json::to_string_pretty(actual).expect("actual artifact must serialize")
    );
    assert_eq!(
        canonical_json_bytes(actual).expect("actual canonical artifact"),
        canonical_json_bytes(&expected).expect("expected canonical artifact"),
        "{label} canonical bytes changed"
    );
}

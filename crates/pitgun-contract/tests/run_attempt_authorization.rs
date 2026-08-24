use pitgun_contract::{
    ArtifactIdentity, AuthorizationSignatureAlgorithm, AuthorizationValidityV1, ContractVersion,
    DeterministicRunContractV1, Digest, EventOrderingV1, ExecutionReceiptV1, Identifier,
    InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1, RandomAlgorithm,
    RandomContractV1, RunAttemptAuthorizationError, RunAttemptAuthorizationV1,
    RunAttemptAuthorizationVersion, RuntimeIdentity, RuntimeProfile, ScenarioIdentity, Seed,
    SemanticVersion, SignedRunAttemptAuthorizationV1, StreamDerivation,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

fn artifact(id: &str, version: &str, bytes: &[u8]) -> ArtifactIdentity {
    ArtifactIdentity {
        id: Identifier::new(id).expect("artifact id"),
        version: SemanticVersion::new(version).expect("artifact version"),
        digest: Digest::from_bytes(bytes),
    }
}

fn initial_contract() -> DeterministicRunContractV1 {
    DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: Identifier::new("racing.weekend").expect("scenario id"),
            version: SemanticVersion::new("1.0.0").expect("scenario version"),
        },
        model: artifact("pitgun.racing-model-v3", "0.13.0", b"model"),
        data_pack: artifact("pitgun.racing", "1.6.0", b"catalog"),
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: Seed::new(42),
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 50_000, 1).expect("logical clock"),
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: Digest::from_bytes(b"initial input"),
        },
    }
}

fn authorization() -> RunAttemptAuthorizationV1 {
    let initial_contract = initial_contract();
    let initial_run_id = initial_contract.run_id().expect("initial run id");
    RunAttemptAuthorizationV1 {
        authorization_version: RunAttemptAuthorizationVersion::V1,
        nonce: Digest::from_bytes(b"nonce"),
        execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
            .parse()
            .expect("execution id"),
        subject: Identifier::new("career.123").expect("subject"),
        audience: Identifier::new("pitgun.verifier").expect("audience"),
        initial_contract,
        initial_run_id,
        decision_envelope: artifact(
            "pitgun.racing-driver-instruction-authorization",
            "1.0.0",
            b"Racing decision envelope",
        ),
        policy: artifact("pitgun.racing.policy", "1.0.0", b"policy"),
        signing_key_id: Identifier::new("staging-2026-08-v2").expect("key id"),
        validity: AuthorizationValidityV1 {
            issued_at_ms: 1_710_000_000_000,
            expires_at_ms: 1_710_000_300_000,
            late_submission_grace_ms: 120_000,
        },
    }
}

fn final_contract() -> DeterministicRunContractV1 {
    let mut final_contract = initial_contract();
    final_contract.input.digest = Digest::from_bytes(b"completed input with decision history");
    final_contract
}

fn runtime() -> RuntimeIdentity {
    RuntimeIdentity {
        engine: Identifier::new("pitgun.racing-wasm").expect("engine"),
        engine_version: SemanticVersion::new("0.13.0").expect("engine version"),
        target: Identifier::new("wasm32-unknown-unknown").expect("target"),
        artifact_digest: Digest::from_bytes(b"wasm"),
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn dynamic_attempt_authorization_is_strict_and_portable() {
    let authorization = authorization();
    authorization
        .validate_integrity()
        .expect("valid attempt authorization");
    let bytes = authorization
        .signing_bytes()
        .expect("canonical signing bytes");

    let signed = SignedRunAttemptAuthorizationV1 {
        authorization: authorization.clone(),
        algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
        signature: "00".repeat(32),
    };
    let encoded = serde_json::to_vec(&signed).expect("signed JSON");
    let decoded: SignedRunAttemptAuthorizationV1 =
        serde_json::from_slice(&encoded).expect("strict signed authorization");
    assert_eq!(decoded, signed);
    assert_eq!(decoded.authorization.signing_bytes().unwrap(), bytes);

    let mut changed = authorization;
    changed.decision_envelope.digest = Digest::from_bytes(b"other envelope");
    assert_ne!(changed.signing_bytes().unwrap(), bytes);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn completed_contract_and_receipt_are_bound_to_the_authorized_attempt() {
    let authorization = authorization();
    let final_contract = final_contract();
    let receipt = ExecutionReceiptV1::for_contract(
        &final_contract,
        authorization.execution_id,
        runtime(),
        Digest::from_bytes(b"output"),
        Digest::from_bytes(b"telemetry"),
    )
    .expect("receipt");

    authorization
        .validate_completed_receipt(&final_contract, &receipt)
        .expect("authorized completed receipt");

    let mut wrong_model = final_contract.clone();
    wrong_model.model.digest = Digest::from_bytes(b"other model");
    assert!(matches!(
        authorization.validate_final_contract(&wrong_model),
        Err(RunAttemptAuthorizationError::FinalContractSemanticMismatch)
    ));

    let wrong_execution = ExecutionReceiptV1::for_contract(
        &final_contract,
        "018f3b78-7e9a-7d20-a5e1-4ed92f02a592"
            .parse()
            .expect("other execution id"),
        runtime(),
        Digest::from_bytes(b"output"),
        Digest::from_bytes(b"telemetry"),
    )
    .expect("wrong-attempt receipt");
    assert!(matches!(
        authorization.validate_completed_receipt(&final_contract, &wrong_execution),
        Err(RunAttemptAuthorizationError::ExecutionIdMismatch { .. })
    ));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn forged_initial_identity_and_wire_extensions_fail_closed() {
    let mut forged = authorization();
    forged.initial_run_id = Digest::from_bytes(b"forged");
    assert!(matches!(
        forged.signing_bytes(),
        Err(RunAttemptAuthorizationError::InitialRunIdMismatch { .. })
    ));

    let mut encoded = serde_json::to_value(authorization()).expect("authorization JSON");
    encoded["final_run_id"] = serde_json::json!(Digest::from_bytes(b"premature final id"));
    assert!(serde_json::from_value::<RunAttemptAuthorizationV1>(encoded).is_err());
}

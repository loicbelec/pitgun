use pitgun_contract::{
    CanonicalJsonError, DeterministicRunContractV1, Digest, ExecutionReceiptV1, Identifier,
    RuntimeIdentity, SemanticVersion, canonical_json_digest, canonicalize_json_str,
};
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn canonical_bytes_and_digest_match_the_published_vector() {
    let canonical =
        canonicalize_json_str(r#"{"c":120,"a":"Hello!","b":false}"#).expect("canonical JSON");
    let digest = canonical_json_digest(&json!({"a": "Hello!", "b": false, "c": 120}))
        .expect("canonical digest");

    assert_eq!(canonical, br#"{"a":"Hello!","b":false,"c":120}"#);
    assert_eq!(
        digest.to_string(),
        "sha256:eade68c3df1936c19da4955a9e876474d4b3e52e016e6b02ed354d9c42a49513"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn strict_input_failures_match_across_runtimes() {
    let duplicate =
        canonicalize_json_str(r#"{"value":1,"value":2}"#).expect_err("duplicate key must fail");
    let unsafe_integer = canonicalize_json_str(r#"{"value":9007199254740992}"#)
        .expect_err("unsafe integer must fail");

    assert!(matches!(duplicate, CanonicalJsonError::DuplicateKey(_)));
    assert!(matches!(
        unsafe_integer,
        CanonicalJsonError::UnsafeInteger { .. }
    ));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn digest_text_round_trip_matches_across_runtimes() {
    let digest = Digest::from_bytes(b"pitgun-determinism-v1");
    let parsed: Digest = digest.to_string().parse().expect("canonical digest text");

    assert_eq!(parsed, digest);
}

const RACING_CONTRACT: &str = r#"
{
  "input": {
    "digest": "sha256:23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01",
    "canonicalization": "jcs-rfc8785",
    "media_type": "application/json"
  },
  "clock": {
    "tick_denominator": 1,
    "epoch": 0,
    "kind": "logical-fixed-step",
    "tick_numerator_us": 50000
  },
  "contract_version": "pitgun.deterministic-run/v1",
  "event_ordering": {
    "string_order": "unicode-code-point",
    "keys": ["logical_tick", "source_id", "source_sequence", "insertion_ordinal"]
  },
  "random": {
    "stream_derivation": "sha256-label-v1",
    "algorithm": "pitgun-splitmix64-v1",
    "seed": "7"
  },
  "runtime_profile": "portable-exact-v1",
  "data_pack": {
    "digest": "sha256:123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0",
    "version": "1.0.0",
    "id": "pitgun.racing.2026"
  },
  "model": {
    "version": "1.0.0",
    "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "id": "pitgun.racing"
  },
  "scenario": {"version": "1.0.0", "id": "racing.single-lap"}
}
"#;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_contract_run_id_and_receipts_match_across_runtimes() {
    let contract: DeterministicRunContractV1 =
        serde_json::from_str(RACING_CONTRACT).expect("Racing reference contract");
    let run_id = contract.run_id().expect("Racing run id");

    let native = receipt(&contract, "aarch64-apple-darwin", 1);
    let wasm = receipt(&contract, "wasm32-unknown-unknown", 2);

    assert_eq!(
        run_id.to_string(),
        "sha256:b62d0883c266f3927eaf3d0743edcc3d25508338c010ef759a005e0b506b6386"
    );
    assert_eq!(native.run_id, run_id);
    assert_eq!(wasm.run_id, run_id);
    assert_ne!(native.execution_id, wasm.execution_id);
    assert_ne!(native.runtime, wasm.runtime);
}

fn receipt(contract: &DeterministicRunContractV1, target: &str, ordinal: u8) -> ExecutionReceiptV1 {
    let execution_id = format!("018f3b78-7e9a-7d20-a5e1-4ed92f02a59{ordinal}")
        .parse()
        .expect("execution id");
    ExecutionReceiptV1::for_contract(
        contract,
        execution_id,
        RuntimeIdentity {
            engine: Identifier::new("pitgun-rust").expect("engine"),
            engine_version: SemanticVersion::new("1.0.0").expect("engine version"),
            target: Identifier::new(target).expect("target"),
            artifact_digest: Digest::from_bytes(target.as_bytes()),
        },
        Digest::from_bytes(b"Racing output"),
        Digest::from_bytes(b"Racing telemetry summary"),
    )
    .expect("execution receipt")
}

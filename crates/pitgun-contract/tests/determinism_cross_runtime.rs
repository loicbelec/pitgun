use pitgun_contract::{CanonicalJsonError, Digest, canonical_json_digest, canonicalize_json_str};
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

use pitgun_contract::{ArtifactIdentity, Digest};
use pitgun_racing_contract::{
    RacingModelParametersError, RacingModelParametersPurpose, RacingModelParametersV1,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const FIXTURE_JSON: &str = include_str!("fixtures/racing_model_parameters_v1.json");

fn fixture() -> RacingModelParametersV1 {
    serde_json::from_str(FIXTURE_JSON).expect("reviewed Model V2 compatibility resource")
}

fn model(id: &str, version: &str) -> ArtifactIdentity {
    ArtifactIdentity {
        id: id.parse().expect("model ID"),
        version: version.parse().expect("model version"),
        digest: Digest::from_bytes(format!("{id}:{version}").as_bytes()),
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn compatibility_fixture_is_strict_valid_and_canonical() {
    let resource = fixture();
    resource.validate().expect("valid parameter resource");
    assert_eq!(
        resource.purpose,
        RacingModelParametersPurpose::ModelV2Compatibility
    );
    assert_eq!(
        resource
            .canonical_digest()
            .expect("canonical digest")
            .to_string(),
        "sha256:1c60391e5c536248153b5cae8608bc126f85b5ca31fe04b5cc84a424673e3f50"
    );

    let canonical = pitgun_contract::canonical_json_bytes(&resource).expect("canonical JSON");
    let reparsed: RacingModelParametersV1 =
        serde_json::from_slice(&canonical).expect("canonical resource reparses");
    assert_eq!(reparsed, resource);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn compatibility_is_exact_and_fails_closed() {
    let resource = fixture();
    resource
        .validate_for_model(&model("pitgun.racing", "2.0.0"))
        .expect("exact Model V2 compatibility");

    assert!(matches!(
        resource.validate_for_model(&model("pitgun.racing", "3.0.0")),
        Err(RacingModelParametersError::UnsupportedModel { .. })
    ));
    assert!(matches!(
        resource.validate_for_model(&model("pitgun.racing-reference", "2.0.0")),
        Err(RacingModelParametersError::UnsupportedModel { .. })
    ));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn numeric_bounds_and_joint_invariants_reject_invalid_resources() {
    let mut invalid_cap = fixture();
    invalid_cap.development_resolution.points_cap_per_axis = 0.0;
    assert!(matches!(
        invalid_cap.validate(),
        Err(RacingModelParametersError::InvalidParameter { .. })
    ));

    let mut non_finite = fixture();
    non_finite.setup_response.drag_area_slider_gain = f64::NAN;
    assert!(matches!(
        non_finite.validate(),
        Err(RacingModelParametersError::InvalidParameter { .. })
    ));

    let mut inverted_gearing = fixture();
    inverted_gearing.setup_response.gear_ratio_slider_reduction =
        inverted_gearing.setup_response.gear_ratio_base_multiplier;
    assert!(matches!(
        inverted_gearing.validate(),
        Err(RacingModelParametersError::InvalidRelationship(_))
    ));
}

#[test]
fn unknown_or_missing_fields_are_rejected() {
    let mut unknown: serde_json::Value = serde_json::from_str(FIXTURE_JSON).expect("fixture JSON");
    unknown
        .as_object_mut()
        .expect("resource object")
        .insert("mutable_latest".to_string(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<RacingModelParametersV1>(unknown).is_err());

    let mut missing: serde_json::Value = serde_json::from_str(FIXTURE_JSON).expect("fixture JSON");
    missing
        .as_object_mut()
        .expect("resource object")
        .remove("setup_response");
    assert!(serde_json::from_value::<RacingModelParametersV1>(missing).is_err());
}

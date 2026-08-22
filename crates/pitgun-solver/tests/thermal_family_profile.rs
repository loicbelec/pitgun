use pitgun_contract::canonical_json_bytes;
use pitgun_solver::{
    RaceOutput, RacingCatalogBundleV1, RacingCatalogFileV1, RunRaceRequest,
    V3_POWER_UNIT_THERMAL_PROFILE_DIGEST, V3_THERMAL_FAMILY_PROFILE_DIGEST,
    V3PowerUnitThermalProfileCandidateV2, V3ThermalFamilyProfileCandidateV1,
    run_race_with_catalog_and_v3_power_unit_thermal_profile,
    run_race_with_catalog_and_v3_power_unit_thermal_profile_json,
    run_race_with_catalog_and_v3_thermal_family_profile,
    run_race_with_catalog_and_v3_thermal_family_profile_json,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const INPUT: &str =
    include_str!("../../pitgun-racing-simulator/tests/golden/racing_run_v2.input.json");
const CANDIDATE: &str = include_str!(
    "../../../experiments/racing_v3_thermal_refinement/candidates/thermal-family-profile-v1.json"
);
const POWER_UNIT_CANDIDATE: &str = include_str!(
    "../../../experiments/racing_v3_thermal_refinement/candidates/thermal-family-profile-v2.json"
);

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/racing_catalog_v1.rs"
));

fn text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .expect("Racing Catalog V1 is UTF-8 JSON")
        .to_owned()
}

fn bundle() -> RacingCatalogBundleV1 {
    RacingCatalogBundleV1 {
        manifest: include_str!("../../../catalogs/racing/v1.0.0/catalog.json").to_owned(),
        release_identity: include_str!("../../../catalogs/racing/v1.0.0/release.json").to_owned(),
        simulation_index: include_str!("../../../catalogs/racing/v1.0.0/simulation/index.json")
            .to_owned(),
        presentation_index: include_str!("../../../catalogs/racing/v1.0.0/presentation/index.json")
            .to_owned(),
        resources: EMBEDDED_FILES
            .iter()
            .map(|(path, contents)| RacingCatalogFileV1 {
                path: format!("simulation/{path}"),
                contents: text(contents),
            })
            .collect(),
    }
}

fn request_for(vehicle_id: &str) -> RunRaceRequest {
    let mut request = serde_json::from_str::<RunRaceRequest>(INPUT).expect("Racing request");
    request.input.vehicle_id = Some(vehicle_id.to_string());
    request
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn reviewed_candidate_resolves_every_family_identically_in_rust_and_json_facade() {
    let candidate =
        V3ThermalFamilyProfileCandidateV1::from_exact_json(CANDIDATE).expect("candidate");
    let catalog_bundle = bundle();
    let catalog_bundle_json = serde_json::to_string(&catalog_bundle).expect("catalog bundle JSON");
    let catalog = pitgun_solver::RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json)
        .expect("catalog");

    for (vehicle_id, expected_family) in [
        ("classic_v8_1960", "historical_v8"),
        ("classic_v8_1970", "historical_v8"),
        ("modern_v6t", "modern_v6t"),
        ("f1_2026", "f1_2026"),
    ] {
        let request = request_for(vehicle_id);
        let native = run_race_with_catalog_and_v3_thermal_family_profile(
            request.clone(),
            &catalog,
            &candidate,
        )
        .expect("native thermal candidate run");
        let facade_json = run_race_with_catalog_and_v3_thermal_family_profile_json(
            serde_json::to_string(&request).expect("request JSON"),
            catalog_bundle_json.clone(),
            CANDIDATE.to_string(),
        );
        let facade = serde_json::from_str::<RaceOutput>(&facade_json)
            .unwrap_or_else(|error| panic!("thermal JSON facade failed: {error}: {facade_json}"));

        assert_eq!(
            canonical_json_bytes(&native).expect("native canonical output"),
            canonical_json_bytes(&facade).expect("facade canonical output"),
            "native and WASM-compatible JSON boundaries differ for {vehicle_id}",
        );
        let resolution = native
            .player_thermal_family_resolution_v3
            .expect("thermal resolution evidence");
        assert_eq!(resolution.family, expected_family);
        assert_eq!(resolution.vehicle_id, vehicle_id);
        assert_eq!(
            resolution.candidate.digest.to_string(),
            V3_THERMAL_FAMILY_PROFILE_DIGEST
        );
        assert_eq!(resolution.model.version.to_string(), "0.10.0");
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn reviewed_candidate_fails_closed_for_unknown_vehicle_and_mutated_bytes() {
    let candidate =
        V3ThermalFamilyProfileCandidateV1::from_exact_json(CANDIDATE).expect("candidate");
    let catalog_bundle_json = serde_json::to_string(&bundle()).expect("catalog bundle JSON");
    let catalog = pitgun_solver::RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json)
        .expect("catalog");
    let unknown = run_race_with_catalog_and_v3_thermal_family_profile(
        request_for("default"),
        &catalog,
        &candidate,
    )
    .expect_err("unreviewed vehicle must fail closed");
    assert!(unknown.contains("unknown vehicle \"default\" for thermal family profile"));

    let mut mutated = CANDIDATE.to_string();
    mutated.push(' ');
    let error = V3ThermalFamilyProfileCandidateV1::from_exact_json(&mutated)
        .expect_err("mutated candidate bytes must be rejected");
    assert!(error.contains("unsupported thermal family profile digest"));

    let facade = run_race_with_catalog_and_v3_thermal_family_profile_json(
        serde_json::to_string(&request_for("default")).expect("request JSON"),
        catalog_bundle_json,
        CANDIDATE.to_string(),
    );
    assert!(
        facade.contains("unknown vehicle"),
        "unexpected facade error: {facade}"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn component_candidate_resolves_each_competitor_power_unit_identically_across_boundaries() {
    let candidate = V3PowerUnitThermalProfileCandidateV2::from_exact_json(POWER_UNIT_CANDIDATE)
        .expect("power-unit candidate");
    let catalog_bundle = bundle();
    let catalog_bundle_json = serde_json::to_string(&catalog_bundle).expect("catalog bundle JSON");
    let catalog = pitgun_solver::RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json)
        .expect("catalog");
    let mut request = request_for("f1_2026");
    let mut opponent = request.input.race.competitors[0].clone();
    opponent.id = "opponent".to_string();
    opponent.name = "Opponent".to_string();
    opponent.is_player = false;
    request.input.race.competitors.push(opponent);
    request.input.competitor_vehicle_components.insert(
        "player".to_string(),
        serde_json::from_value(serde_json::json!({
            "schema_version": "pitgun.racing-vehicle-components/v1",
            "engine_id": "v6t_hybrid"
        }))
        .expect("player components"),
    );
    request.input.competitor_vehicle_components.insert(
        "opponent".to_string(),
        serde_json::from_value(serde_json::json!({
            "schema_version": "pitgun.racing-vehicle-components/v1",
            "engine_id": "v6t"
        }))
        .expect("opponent components"),
    );

    let native = run_race_with_catalog_and_v3_power_unit_thermal_profile(
        request.clone(),
        &catalog,
        &candidate,
    )
    .expect("native component candidate run");
    let facade_json = run_race_with_catalog_and_v3_power_unit_thermal_profile_json(
        serde_json::to_string(&request).expect("request JSON"),
        catalog_bundle_json,
        POWER_UNIT_CANDIDATE.to_string(),
    );
    let facade = serde_json::from_str::<RaceOutput>(&facade_json)
        .unwrap_or_else(|error| panic!("component JSON facade failed: {error}: {facade_json}"));

    assert_eq!(
        canonical_json_bytes(&native).expect("native canonical output"),
        canonical_json_bytes(&facade).expect("facade canonical output"),
        "native and WASM-compatible component boundaries differ",
    );
    let player = native
        .competitor_power_unit_thermal_resolutions_v3
        .get("player")
        .expect("player resolution");
    let opponent = native
        .competitor_power_unit_thermal_resolutions_v3
        .get("opponent")
        .expect("opponent resolution");
    assert_eq!(player.power_unit_id, "v6t_hybrid");
    assert_eq!(player.family, "f1_2026");
    assert_eq!(opponent.power_unit_id, "v6t");
    assert_eq!(opponent.family, "modern_v6t");
    assert_eq!(
        player.candidate.digest.to_string(),
        V3_POWER_UNIT_THERMAL_PROFILE_DIGEST
    );
    assert_eq!(player.model.version.to_string(), "0.11.0");
    assert_eq!(
        native.player_power_unit_thermal_resolution_v3.as_ref(),
        Some(player)
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn component_candidate_fails_closed_for_unknown_power_unit_and_mutated_bytes() {
    let candidate = V3PowerUnitThermalProfileCandidateV2::from_exact_json(POWER_UNIT_CANDIDATE)
        .expect("power-unit candidate");
    assert!(candidate.resolve_power_unit("v8_1960").is_ok());
    assert!(candidate.resolve_power_unit("v8_1970").is_ok());
    assert!(candidate.resolve_power_unit("v6t").is_ok());
    assert!(candidate.resolve_power_unit("v6t_hybrid").is_ok());
    let unknown = candidate
        .resolve_power_unit("unreviewed-prototype")
        .expect_err("unknown power unit must fail closed");
    assert!(unknown.contains("unknown power unit"));

    let mut mutated = POWER_UNIT_CANDIDATE.to_string();
    mutated.push(' ');
    let error = V3PowerUnitThermalProfileCandidateV2::from_exact_json(&mutated)
        .expect_err("mutated candidate bytes must be rejected");
    assert!(error.contains("unsupported power-unit thermal profile digest"));
}

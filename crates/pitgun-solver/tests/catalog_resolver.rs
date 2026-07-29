use pitgun_solver::{RacingCatalogBundleV1, RacingCatalogFileV1, RacingCatalogSnapshot};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn browser_bundle_resolves_portably() {
    let bundle_json = serde_json::to_string(&bundle()).expect("bundle JSON");
    let resolved = RacingCatalogSnapshot::from_bundle_json(&bundle_json).expect("catalog");

    assert_eq!(resolved.manifest().catalog.id.to_string(), "pitgun.racing");
    assert_eq!(resolved.manifest().catalog.version.to_string(), "1.0.0");
    assert_eq!(
        resolved.release_identity().manifest_digest.to_string(),
        "sha256:5f36612a3ce265750051ca913747c0c05bc08a534dc307bbc7b02b8cb2f6e156"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn browser_bundle_rejects_mutated_resource_bytes_portably() {
    let mut bundle = bundle();
    bundle.resources[0].contents.push(' ');
    let bundle_json = serde_json::to_string(&bundle).expect("bundle JSON");

    assert!(RacingCatalogSnapshot::from_bundle_json(&bundle_json).is_err());
}

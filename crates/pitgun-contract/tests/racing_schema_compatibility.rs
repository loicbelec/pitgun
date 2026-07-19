use pitgun_contract::{
    CircuitCatalogEntry, Digest, EngineCatalogEntry, RaceInput, RaceOutput, RunPackage,
    SignedSimulationContractV1, VehicleClass, canonical_json_bytes, canonical_json_digest,
};
use pitgun_signing::SigningKey;
use serde::Deserialize;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const FIXTURE_JSON: &str = include_str!("fixtures/racing_contract_v1.json");
const FIXTURE_SCHEMA: &str = "pitgun.racing-contract-compatibility/v1";
const SIGNING_SECRET: &[u8] = b"pitgun-racing-contract-compatibility-v1";

#[derive(Debug, Deserialize)]
struct CompatibilityFixture {
    schema_version: String,
    race_input: RaceInput,
    race_input_digest: String,
    race_output: RaceOutput,
    race_output_digest: String,
    run_package: RunPackage,
    run_package_digest: String,
    vehicle_class: VehicleClass,
    vehicle_class_digest: String,
    circuit_catalog_entry: CircuitCatalogEntry,
    circuit_catalog_entry_digest: String,
    engine_catalog_entry: EngineCatalogEntry,
    engine_catalog_entry_digest: String,
    signed_simulation_contract: SignedSimulationContractV1,
    signed_simulation_contract_digest: String,
    simulation_contract_signing_json: String,
    simulation_contract_signing_digest: String,
}

fn fixture() -> CompatibilityFixture {
    serde_json::from_str(FIXTURE_JSON).expect("published Racing compatibility fixture")
}

fn assert_canonical_digest<T: serde::Serialize>(value: &T, expected: &str) {
    let canonical = canonical_json_bytes(value).expect("canonical JSON");
    let digest = canonical_json_digest(value).expect("canonical digest");
    assert_eq!(digest.to_string(), expected);

    let reparsed: serde_json::Value = serde_json::from_slice(&canonical).expect("canonical value");
    assert_eq!(
        canonical_json_bytes(&reparsed).expect("recanonicalized JSON"),
        canonical
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_payloads_match_the_published_canonical_digests() {
    let fixture = fixture();
    assert_eq!(fixture.schema_version, FIXTURE_SCHEMA);

    assert_canonical_digest(&fixture.race_input, &fixture.race_input_digest);
    assert_canonical_digest(&fixture.race_output, &fixture.race_output_digest);
    assert_canonical_digest(&fixture.run_package, &fixture.run_package_digest);
    assert_canonical_digest(&fixture.vehicle_class, &fixture.vehicle_class_digest);
    assert_canonical_digest(
        &fixture.circuit_catalog_entry,
        &fixture.circuit_catalog_entry_digest,
    );
    assert_canonical_digest(
        &fixture.engine_catalog_entry,
        &fixture.engine_catalog_entry_digest,
    );
    assert_canonical_digest(
        &fixture.signed_simulation_contract,
        &fixture.signed_simulation_contract_digest,
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn signed_simulation_contract_matches_published_signing_bytes() {
    let fixture = fixture();
    let signing_bytes = fixture
        .signed_simulation_contract
        .contract
        .signing_bytes()
        .expect("simulation contract signing bytes");

    assert_eq!(
        std::str::from_utf8(&signing_bytes).expect("signing JSON is UTF-8"),
        fixture.simulation_contract_signing_json
    );
    assert_eq!(
        Digest::from_bytes(&signing_bytes).to_string(),
        fixture.simulation_contract_signing_digest
    );

    let key = SigningKey::from_secret(SIGNING_SECRET).expect("fixture signing key");
    assert!(key.verify(
        &signing_bytes,
        &fixture.signed_simulation_contract.signature
    ));
}

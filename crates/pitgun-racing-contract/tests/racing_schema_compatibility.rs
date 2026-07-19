use pitgun_racing_contract::{
    CircuitCatalogEntry, EngineCatalogEntry, RaceInput, RaceOutput, RunPackage,
    SignedSimulationContractV1, VehicleClass,
};
use pitgun_signing::SigningKey;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const FIXTURE_JSON: &str =
    include_str!("../../pitgun-contract/tests/fixtures/racing_contract_v1.json");
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

fn canonical_bytes<T: serde::Serialize>(value: &T) -> Vec<u8> {
    serde_json_canonicalizer::to_vec(value).expect("canonical JSON")
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn assert_canonical_digest<T: serde::Serialize>(value: &T, expected: &str) {
    let canonical = canonical_bytes(value);
    assert_eq!(sha256(&canonical), expected);

    let reparsed: serde_json::Value = serde_json::from_slice(&canonical).expect("canonical value");
    assert_eq!(canonical_bytes(&reparsed), canonical);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn racing_payloads_keep_the_pre_migration_canonical_digests() {
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
fn authority_signing_bytes_keep_the_pre_migration_identity() {
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
        sha256(&signing_bytes),
        fixture.simulation_contract_signing_digest
    );

    let key = SigningKey::from_secret(SIGNING_SECRET).expect("fixture signing key");
    assert!(key.verify(
        &signing_bytes,
        &fixture.signed_simulation_contract.signature
    ));
}

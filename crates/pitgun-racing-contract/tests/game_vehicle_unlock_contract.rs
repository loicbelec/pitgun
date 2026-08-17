use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const FIXTURE_JSON: &str = include_str!("fixtures/game_vehicle_unlock_contract_v1.json");
const FIXTURE_SCHEMA: &str = "pitgun.racing-game-vehicle-contract/v1";

#[derive(Debug, Deserialize)]
struct Fixture {
    schema_version: String,
    contract_id: String,
    contract_version: String,
    public_release_max_era: u8,
    catalog: CatalogIdentity,
    unlock_rules: Vec<UnlockRule>,
    allowed_vehicle_ids_by_enabled_era: serde_json::Map<String, serde_json::Value>,
    disabled_eras: Vec<u8>,
    rejected_unreviewed_vehicle_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CatalogIdentity {
    id: String,
    version: String,
    simulation_pack_digest: String,
    model_id: String,
    model_version: String,
}

#[derive(Debug, Deserialize)]
struct UnlockRule {
    vehicle_id: String,
    min_era: u8,
    required_upgrades: Vec<String>,
    components: VehicleComponents,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct VehicleComponents {
    engine: String,
    aero: String,
    chassis: String,
    tire: String,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("framework repository root")
}

fn read_json(path: impl AsRef<Path>) -> serde_json::Value {
    let path = path.as_ref();
    serde_json::from_slice(
        &fs::read(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

#[test]
fn every_enabled_game_vehicle_resolves_to_reviewed_catalog_components() {
    assert_eq!(
        hex::encode(Sha256::digest(FIXTURE_JSON.as_bytes())),
        "4e26ede1e986fb302c6ba700136b9ece56c68d712b72aa7d962d317e807399e4"
    );
    let fixture: Fixture = serde_json::from_str(FIXTURE_JSON).expect("vehicle contract fixture");
    assert_eq!(fixture.schema_version, FIXTURE_SCHEMA);
    assert_eq!(fixture.contract_id, "pitgun.racing.game-vehicle-unlocks");
    assert_eq!(fixture.contract_version, "1.0.0");
    assert_eq!(fixture.public_release_max_era, 5);
    assert_eq!(fixture.disabled_eras, [6, 7]);
    assert_eq!(fixture.catalog.model_id, "pitgun.racing");
    assert_eq!(fixture.catalog.model_version, "2.0.0");

    let catalog_root = repository_root()
        .join("catalogs/racing")
        .join(format!("v{}", fixture.catalog.version));
    let catalog = read_json(catalog_root.join("catalog.json"));
    assert_eq!(catalog["catalog"]["id"], fixture.catalog.id);
    assert_eq!(catalog["catalog"]["version"], fixture.catalog.version);
    assert_eq!(
        catalog["simulation_pack"]["identity"]["digest"],
        fixture.catalog.simulation_pack_digest
    );

    let index = read_json(catalog_root.join("simulation/index.json"));
    let resources = index["resources"]
        .as_array()
        .expect("simulation index resources");
    let resource_ids = resources
        .iter()
        .map(|resource| resource["id"].as_str().expect("resource id"))
        .collect::<BTreeSet<_>>();

    for rule in &fixture.unlock_rules {
        assert!(rule.min_era <= fixture.public_release_max_era);
        assert!(
            resource_ids.contains(format!("pitgun.racing.vehicles.{}", rule.vehicle_id).as_str()),
            "vehicle {} is missing from the pinned catalog",
            rule.vehicle_id
        );
        assert!(
            rule.required_upgrades
                .iter()
                .all(|upgrade| !upgrade.trim().is_empty())
        );

        let actual: VehicleComponents = serde_json::from_value(read_json(
            catalog_root
                .join("simulation/vehicles")
                .join(format!("{}.json", rule.vehicle_id)),
        ))
        .expect("vehicle components");
        assert_eq!(actual, rule.components, "vehicle {}", rule.vehicle_id);

        for (kind, component) in [
            ("engines", &rule.components.engine),
            ("aero", &rule.components.aero),
            ("chassis", &rule.components.chassis),
            ("tires", &rule.components.tire),
        ] {
            assert!(
                resource_ids.contains(format!("pitgun.racing.{kind}.{component}").as_str()),
                "vehicle {} references missing {kind} component {component}",
                rule.vehicle_id
            );
        }
    }

    let unlocks = fixture
        .unlock_rules
        .iter()
        .map(|rule| (rule.vehicle_id.as_str(), rule.min_era))
        .collect::<Vec<_>>();
    for era in 1..=fixture.public_release_max_era {
        let expected = unlocks
            .iter()
            .filter(|(_, min_era)| *min_era <= era)
            .map(|(vehicle_id, _)| *vehicle_id)
            .collect::<Vec<_>>();
        let actual = fixture.allowed_vehicle_ids_by_enabled_era[&era.to_string()]
            .as_array()
            .expect("allowed vehicles for enabled era")
            .iter()
            .map(|value| value.as_str().expect("allowed vehicle id"))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "enabled Era {era}");
    }

    for rejected in &fixture.rejected_unreviewed_vehicle_ids {
        assert!(
            !resource_ids.contains(format!("pitgun.racing.vehicles.{rejected}").as_str()),
            "unreviewed vehicle {rejected} must not enter the immutable catalog"
        );
    }
}

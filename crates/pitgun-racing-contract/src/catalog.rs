//! Versioned Racing-specific catalog index contracts.

use std::fmt;

use pitgun_contract::CatalogResourceV1;
use serde::{Deserialize, Serialize};

/// Wire version of the Racing Simulation Pack index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingSimulationIndexVersion {
    /// First immutable Racing simulation-resource index.
    #[serde(rename = "pitgun.racing-simulation-index/v1")]
    V1,
}

/// Wire version of the Racing Presentation Pack index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingPresentationIndexVersion {
    /// First immutable Racing presentation-resource index.
    #[serde(rename = "pitgun.racing-presentation-index/v1")]
    V1,
}

/// Content-addressed resources that may influence Racing execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingSimulationIndexV1 {
    /// Exact index schema version.
    pub schema_version: RacingSimulationIndexVersion,
    /// Resources ordered by stable resource identifier.
    pub resources: Vec<CatalogResourceV1>,
}

/// Browser-facing presentation of one Racing circuit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingCircuitPresentationV1 {
    /// Stable lowercase filename stem in the Simulation Pack.
    pub source_id: String,
    /// Existing browser identifier retained for game compatibility.
    pub id: String,
    /// Stable model identifier parsed by the simulator.
    pub model_id: String,
    /// Human-readable circuit name.
    pub display_name: String,
    /// Optional ISO-like two-letter country code.
    pub country_code: Option<String>,
    /// Optional default race distance.
    pub laps: Option<u16>,
}

/// Browser-facing presentation of one Racing driver.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingDriverPresentationV1 {
    /// Stable driver identifier.
    pub id: String,
    /// Human-readable driver name.
    pub display_name: String,
}

/// Presentation metadata that must not influence Racing physical execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingPresentationIndexV1 {
    /// Exact index schema version.
    pub schema_version: RacingPresentationIndexVersion,
    /// Circuits ordered by `source_id`.
    pub circuits: Vec<RacingCircuitPresentationV1>,
    /// Drivers ordered by `id`.
    pub drivers: Vec<RacingDriverPresentationV1>,
}

/// Structural failure in a Racing catalog index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RacingCatalogError {
    /// An index contains no resources.
    EmptySimulationPack,
    /// Resource identifiers or paths are duplicated or not canonically ordered.
    NonCanonicalResources,
    /// A simulation resource is outside the immutable simulation directory.
    InvalidSimulationPath(String),
    /// Circuit presentation entries are duplicated or not ordered by source ID.
    NonCanonicalCircuits,
    /// Driver presentation entries are duplicated or not ordered by ID.
    NonCanonicalDrivers,
    /// A required presentation value is empty or malformed.
    InvalidPresentation {
        /// Resource kind.
        kind: &'static str,
        /// Stable resource ID.
        id: String,
        /// Invalid field.
        field: &'static str,
    },
}

impl fmt::Display for RacingCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySimulationPack => {
                formatter.write_str("Racing Simulation Pack must contain resources")
            }
            Self::NonCanonicalResources => formatter.write_str(
                "Racing simulation resources must have unique, canonically ordered IDs and paths",
            ),
            Self::InvalidSimulationPath(path) => {
                write!(
                    formatter,
                    "Racing simulation resource is outside simulation/: {path}"
                )
            }
            Self::NonCanonicalCircuits => formatter.write_str(
                "Racing circuit presentation entries must be ordered by unique source IDs",
            ),
            Self::NonCanonicalDrivers => formatter
                .write_str("Racing driver presentation entries must be ordered by unique IDs"),
            Self::InvalidPresentation { kind, id, field } => {
                write!(formatter, "Racing {kind} {id:?} has invalid {field}")
            }
        }
    }
}

impl std::error::Error for RacingCatalogError {}

impl RacingSimulationIndexV1 {
    /// Validates deterministic resource ordering and release-relative paths.
    pub fn validate(&self) -> Result<(), RacingCatalogError> {
        if self.resources.is_empty() {
            return Err(RacingCatalogError::EmptySimulationPack);
        }
        if self.resources.windows(2).any(|resources| {
            resources[0].id >= resources[1].id || resources[0].path >= resources[1].path
        }) {
            return Err(RacingCatalogError::NonCanonicalResources);
        }
        if let Some(resource) = self
            .resources
            .iter()
            .find(|resource| !resource.path.as_str().starts_with("simulation/"))
        {
            return Err(RacingCatalogError::InvalidSimulationPath(
                resource.path.to_string(),
            ));
        }
        Ok(())
    }
}

impl RacingPresentationIndexV1 {
    /// Validates stable ordering and required browser-facing values.
    pub fn validate(&self) -> Result<(), RacingCatalogError> {
        if self
            .circuits
            .windows(2)
            .any(|entries| entries[0].source_id >= entries[1].source_id)
        {
            return Err(RacingCatalogError::NonCanonicalCircuits);
        }
        if self
            .drivers
            .windows(2)
            .any(|entries| entries[0].id >= entries[1].id)
        {
            return Err(RacingCatalogError::NonCanonicalDrivers);
        }
        for circuit in &self.circuits {
            if circuit.source_id.trim().is_empty() {
                return Err(invalid_presentation("circuit", &circuit.id, "source_id"));
            }
            if circuit.id.trim().is_empty() {
                return Err(invalid_presentation("circuit", &circuit.id, "id"));
            }
            if circuit.model_id.trim().is_empty() {
                return Err(invalid_presentation("circuit", &circuit.id, "model_id"));
            }
            if circuit.display_name.trim().is_empty() {
                return Err(invalid_presentation("circuit", &circuit.id, "display_name"));
            }
            if circuit.country_code.as_ref().is_some_and(|code| {
                code.len() != 2 || !code.chars().all(|character| character.is_ascii_uppercase())
            }) {
                return Err(invalid_presentation("circuit", &circuit.id, "country_code"));
            }
        }
        for driver in &self.drivers {
            if driver.id.trim().is_empty() {
                return Err(invalid_presentation("driver", &driver.id, "id"));
            }
            if driver.display_name.trim().is_empty() {
                return Err(invalid_presentation("driver", &driver.id, "display_name"));
            }
        }
        Ok(())
    }
}

fn invalid_presentation(kind: &'static str, id: &str, field: &'static str) -> RacingCatalogError {
    RacingCatalogError::InvalidPresentation {
        kind,
        id: id.to_owned(),
        field,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::{
        CatalogReleaseIdentityV1, ResourceCatalogManifestV1, canonical_json_digest,
    };
    #[cfg(not(target_arch = "wasm32"))]
    use sha2::{Digest as _, Sha256};
    #[cfg(not(target_arch = "wasm32"))]
    use std::path::Path;

    const SIMULATION_INDEX: &str =
        include_str!("../../../catalogs/racing/v1.0.0/simulation/index.json");
    const PRESENTATION_INDEX: &str =
        include_str!("../../../catalogs/racing/v1.0.0/presentation/index.json");
    const CATALOG_MANIFEST: &str = include_str!("../../../catalogs/racing/v1.0.0/catalog.json");
    const RELEASE_IDENTITY: &str = include_str!("../../../catalogs/racing/v1.0.0/release.json");

    #[test]
    fn checked_in_release_has_valid_content_identities() {
        let simulation: RacingSimulationIndexV1 =
            serde_json::from_str(SIMULATION_INDEX).expect("simulation index");
        simulation.validate().expect("valid simulation index");

        let presentation: RacingPresentationIndexV1 =
            serde_json::from_str(PRESENTATION_INDEX).expect("presentation index");
        presentation.validate().expect("valid presentation index");

        let manifest: ResourceCatalogManifestV1 =
            serde_json::from_str(CATALOG_MANIFEST).expect("catalog manifest");
        manifest.validate().expect("valid catalog manifest");
        assert_eq!(
            canonical_json_digest(&simulation).expect("simulation digest"),
            manifest.simulation_pack.identity.digest
        );
        assert_eq!(
            canonical_json_digest(&presentation).expect("presentation digest"),
            manifest.presentation_pack.identity.digest
        );

        let release: CatalogReleaseIdentityV1 =
            serde_json::from_str(RELEASE_IDENTITY).expect("release identity");
        manifest
            .verify_release_identity(&release)
            .expect("matching release identity");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn checked_in_simulation_resources_match_their_exact_byte_digests() {
        let index: RacingSimulationIndexV1 =
            serde_json::from_str(SIMULATION_INDEX).expect("simulation index");
        let release_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.0.0");

        for resource in index.resources {
            let bytes = std::fs::read(release_root.join(resource.path.as_str()))
                .expect("indexed simulation resource");
            let actual = format!("sha256:{:x}", Sha256::digest(bytes));
            assert_eq!(actual, resource.digest.to_string(), "{}", resource.path);
        }
    }

    #[test]
    fn checked_in_presentation_index_is_valid_and_strict() {
        let index: RacingPresentationIndexV1 =
            serde_json::from_str(PRESENTATION_INDEX).expect("presentation index");
        index.validate().expect("valid presentation index");

        assert!(index.circuits.iter().any(|circuit| {
            circuit.id == "MONZA" && circuit.display_name == "Autodromo Nazionale Monza"
        }));
        assert!(index.drivers.iter().any(|driver| {
            driver.id == "charles_leclair" && driver.display_name == "Charles L'Eclair"
        }));
    }

    #[test]
    fn unknown_index_versions_fail_closed() {
        let unknown = PRESENTATION_INDEX.replace(
            "pitgun.racing-presentation-index/v1",
            "pitgun.racing-presentation-index/v2",
        );
        assert!(serde_json::from_str::<RacingPresentationIndexV1>(&unknown).is_err());
    }
}

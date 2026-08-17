//! Pure validation and resolution of immutable Racing Catalog releases.
//!
//! This module deliberately accepts bytes supplied by its caller. HTTP,
//! filesystem discovery and mutable `latest` selection remain application
//! concerns and never become part of deterministic execution.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use pitgun_contract::{
    ArtifactIdentity, CatalogPath, CatalogReleaseIdentityV1, ContractVersion,
    DeterministicRunContractV1, Digest, ResolvedScenario, ResourceCatalogManifestV1,
    canonical_json_digest, canonicalize_json_str,
};
use pitgun_racing_contract::{
    RacingModelParametersV1, RacingPresentationIndexV1, RacingSimulationIndexV1,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{EMBEDDED_FILES, MODEL_V2_EMBEDDED_FILES, PRESENTATION_INDEX};

const KNOWN_RACING_MODEL_VERSIONS: [&str; 2] = ["1.0.0", "2.0.0"];
const MODEL_PARAMETERS_ID_PREFIX: &str = "pitgun.racing.model-parameters.";
const MODEL_PARAMETERS_PATH_PREFIX: &str = "simulation/model-parameters/";

const CATALOG_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.0.0/catalog.json"
));
const RELEASE_IDENTITY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.0.0/release.json"
));
const SIMULATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.0.0/simulation/index.json"
));
const MODEL_V2_CATALOG_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.2.0/catalog.json"
));
const MODEL_V2_RELEASE_IDENTITY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.2.0/release.json"
));
const MODEL_V2_SIMULATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.2.0/simulation/index.json"
));
const MODEL_V2_PRESENTATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.2.0/presentation/index.json"
));

/// One exact release-relative file supplied by a browser or another adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingCatalogFileV1 {
    /// Immutable path declared by the Simulation Pack index.
    pub path: String,
    /// Exact UTF-8 JSON file contents.
    pub contents: String,
}

/// Transport-neutral bundle accepted by the browser compatibility facade.
///
/// It is an application interchange shape, not a deterministic Pitgun
/// contract. Every durable identity is recalculated from the contained bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingCatalogBundleV1 {
    /// Exact catalog manifest text.
    pub manifest: String,
    /// Exact release identity text.
    pub release_identity: String,
    /// Exact Simulation Pack index text.
    pub simulation_index: String,
    /// Exact Presentation Pack index text.
    pub presentation_index: String,
    /// Exact Simulation Pack resources.
    pub resources: Vec<RacingCatalogFileV1>,
}

/// Fully validated immutable Racing Catalog release.
#[derive(Clone, Debug)]
pub struct RacingCatalogSnapshot {
    manifest: ResourceCatalogManifestV1,
    release_identity: CatalogReleaseIdentityV1,
    simulation_index: RacingSimulationIndexV1,
    presentation_index: RacingPresentationIndexV1,
    resources: BTreeMap<CatalogPath, Vec<u8>>,
    model_parameters: Option<RacingModelParametersV1>,
}

/// Failure produced before a Racing Catalog may enter simulation.
#[derive(Debug, thiserror::Error)]
pub enum RacingCatalogResolutionError {
    /// One JSON document is not strict, UTF-8 or compatible with its schema.
    #[error("invalid {document}: {reason}")]
    InvalidDocument {
        /// Stable document name.
        document: &'static str,
        /// Parser or schema failure.
        reason: String,
    },
    /// The generic manifest violates a structural or identity invariant.
    #[error("invalid catalog manifest: {0}")]
    InvalidManifest(String),
    /// The Racing pack index violates its domain invariants.
    #[error("invalid Racing {pack} index: {reason}")]
    InvalidIndex {
        /// Pack whose index failed.
        pack: &'static str,
        /// Structural failure.
        reason: String,
    },
    /// Canonical index bytes do not match the manifest.
    #[error("{pack} index digest mismatch: expected {expected}, calculated {actual}")]
    IndexDigestMismatch {
        /// Pack whose index failed.
        pack: &'static str,
        /// Digest declared by the manifest.
        expected: Digest,
        /// Digest calculated from canonical index bytes.
        actual: Digest,
    },
    /// A supplied resource path is not a canonical release-relative path.
    #[error("invalid Racing catalog resource path {path:?}: {reason}")]
    InvalidResourcePath {
        /// Rejected path.
        path: String,
        /// Path validation failure.
        reason: String,
    },
    /// The caller supplied the same path more than once.
    #[error("duplicate Racing catalog resource {0}")]
    DuplicateResource(CatalogPath),
    /// An indexed resource was not supplied.
    #[error("missing Racing catalog resource {0}")]
    MissingResource(CatalogPath),
    /// A supplied resource is not declared by the immutable index.
    #[error("unexpected Racing catalog resource {0}")]
    UnexpectedResource(CatalogPath),
    /// Exact stored resource bytes do not match their declared digest.
    #[error("resource digest mismatch for {path}: expected {expected}, calculated {actual}")]
    ResourceDigestMismatch {
        /// Release-relative resource path.
        path: CatalogPath,
        /// Digest declared by the index.
        expected: Digest,
        /// Digest calculated from the exact supplied bytes.
        actual: Digest,
    },
    /// Racing V1 only accepts JSON physical resources.
    #[error("unsupported media type {media_type} for Racing resource {path}")]
    UnsupportedResourceMediaType {
        /// Release-relative resource path.
        path: CatalogPath,
        /// Declared media type.
        media_type: String,
    },
    /// One physical JSON resource violates the strict input profile.
    #[error("invalid Racing resource {path}: {reason}")]
    InvalidResource {
        /// Release-relative resource path.
        path: CatalogPath,
        /// UTF-8 or strict JSON failure.
        reason: String,
    },
    /// Physical resources or their domain references are incomplete.
    #[error("invalid resolved Racing resources: {0}")]
    InvalidResolvedResources(String),
    /// A run does not bind the model and Simulation Pack selected by this release.
    #[error("Racing catalog is incompatible with the run contract: {0}")]
    IncompatibleRun(String),
    /// A native filesystem adapter could not read one release file.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("cannot read Racing catalog file {path}: {reason}")]
    FileRead {
        /// Filesystem path.
        path: String,
        /// I/O failure.
        reason: String,
    },
}

impl RacingCatalogSnapshot {
    /// Validates one immutable release from caller-provided bytes.
    ///
    /// No simulation state is created until every document, digest, resource
    /// and Racing-specific reference boundary has passed validation.
    pub fn from_bytes(
        manifest_bytes: &[u8],
        release_identity_bytes: &[u8],
        simulation_index_bytes: &[u8],
        presentation_index_bytes: &[u8],
        resources: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self, RacingCatalogResolutionError> {
        let manifest: ResourceCatalogManifestV1 =
            parse_strict_document("catalog manifest", manifest_bytes)?;
        manifest
            .validate()
            .map_err(|error| RacingCatalogResolutionError::InvalidManifest(error.to_string()))?;
        validate_known_racing_model_compatibility(&manifest)?;

        let release_identity: CatalogReleaseIdentityV1 =
            parse_strict_document("release identity", release_identity_bytes)?;
        manifest
            .verify_release_identity(&release_identity)
            .map_err(|error| RacingCatalogResolutionError::InvalidManifest(error.to_string()))?;

        let simulation_index: RacingSimulationIndexV1 =
            parse_strict_document("simulation index", simulation_index_bytes)?;
        simulation_index.validate().map_err(|error| {
            RacingCatalogResolutionError::InvalidIndex {
                pack: "simulation",
                reason: error.to_string(),
            }
        })?;
        verify_index_digest(
            "simulation",
            &simulation_index,
            manifest.simulation_pack.index.digest,
        )?;

        let presentation_index: RacingPresentationIndexV1 =
            parse_strict_document("presentation index", presentation_index_bytes)?;
        presentation_index.validate().map_err(|error| {
            RacingCatalogResolutionError::InvalidIndex {
                pack: "presentation",
                reason: error.to_string(),
            }
        })?;
        verify_index_digest(
            "presentation",
            &presentation_index,
            manifest.presentation_pack.index.digest,
        )?;

        let mut supplied = BTreeMap::new();
        for (path, bytes) in resources {
            let parsed = CatalogPath::new(path.clone()).map_err(|error| {
                RacingCatalogResolutionError::InvalidResourcePath {
                    path,
                    reason: error.to_string(),
                }
            })?;
            if supplied.insert(parsed.clone(), bytes).is_some() {
                return Err(RacingCatalogResolutionError::DuplicateResource(parsed));
            }
        }

        let expected_paths = simulation_index
            .resources
            .iter()
            .map(|resource| resource.path.clone())
            .collect::<BTreeSet<_>>();
        for resource in &simulation_index.resources {
            if resource.media_type.as_str() != "application/json" {
                return Err(RacingCatalogResolutionError::UnsupportedResourceMediaType {
                    path: resource.path.clone(),
                    media_type: resource.media_type.to_string(),
                });
            }
            let bytes = supplied.get(&resource.path).ok_or_else(|| {
                RacingCatalogResolutionError::MissingResource(resource.path.clone())
            })?;
            let text = std::str::from_utf8(bytes).map_err(|error| {
                RacingCatalogResolutionError::InvalidResource {
                    path: resource.path.clone(),
                    reason: error.to_string(),
                }
            })?;
            canonicalize_json_str(text).map_err(|error| {
                RacingCatalogResolutionError::InvalidResource {
                    path: resource.path.clone(),
                    reason: error.to_string(),
                }
            })?;
            let actual = Digest::from_bytes(bytes);
            if actual != resource.digest {
                return Err(RacingCatalogResolutionError::ResourceDigestMismatch {
                    path: resource.path.clone(),
                    expected: resource.digest,
                    actual,
                });
            }
        }
        if let Some(path) = supplied
            .keys()
            .find(|path| !expected_paths.contains(*path))
            .cloned()
        {
            return Err(RacingCatalogResolutionError::UnexpectedResource(path));
        }

        let model_parameters = resolve_model_parameters(&manifest, &simulation_index, &supplied)?;

        let snapshot = Self {
            manifest,
            release_identity,
            simulation_index,
            presentation_index,
            resources: supplied,
            model_parameters,
        };
        crate::EmbeddedCatalog::from_snapshot(&snapshot)
            .map_err(RacingCatalogResolutionError::InvalidResolvedResources)?;
        Ok(snapshot)
    }

    /// Resolves the checked-in offline fallback through the same validator.
    pub fn embedded() -> Result<Self, RacingCatalogResolutionError> {
        let resources = EMBEDDED_FILES
            .iter()
            .map(|(path, bytes)| (format!("simulation/{path}"), bytes.to_vec()));
        Self::from_bytes(
            CATALOG_MANIFEST,
            RELEASE_IDENTITY,
            SIMULATION_INDEX,
            PRESENTATION_INDEX,
            resources,
        )
    }

    /// Resolves the immutable catalog release governed for Racing model V2.
    ///
    /// This does not change the mutable public `LATEST` selection or the V1
    /// offline fallback.
    pub fn embedded_model_v2() -> Result<Self, RacingCatalogResolutionError> {
        let resources = MODEL_V2_EMBEDDED_FILES
            .iter()
            .map(|(path, bytes)| (format!("simulation/{path}"), bytes.to_vec()));
        Self::from_bytes(
            MODEL_V2_CATALOG_MANIFEST,
            MODEL_V2_RELEASE_IDENTITY,
            MODEL_V2_SIMULATION_INDEX,
            MODEL_V2_PRESENTATION_INDEX,
            resources,
        )
    }

    /// Resolves a bundle assembled by a browser or another byte transport.
    pub fn from_bundle(
        bundle: RacingCatalogBundleV1,
    ) -> Result<Self, RacingCatalogResolutionError> {
        Self::from_bytes(
            bundle.manifest.as_bytes(),
            bundle.release_identity.as_bytes(),
            bundle.simulation_index.as_bytes(),
            bundle.presentation_index.as_bytes(),
            bundle
                .resources
                .into_iter()
                .map(|file| (file.path, file.contents.into_bytes())),
        )
    }

    /// Parses and resolves the browser interchange bundle.
    pub fn from_bundle_json(bundle_json: &str) -> Result<Self, RacingCatalogResolutionError> {
        let canonical = canonicalize_json_str(bundle_json).map_err(|error| {
            RacingCatalogResolutionError::InvalidDocument {
                document: "Racing catalog bundle",
                reason: error.to_string(),
            }
        })?;
        let bundle = serde_json::from_slice(&canonical).map_err(|error| {
            RacingCatalogResolutionError::InvalidDocument {
                document: "Racing catalog bundle",
                reason: error.to_string(),
            }
        })?;
        Self::from_bundle(bundle)
    }

    /// Loads an immutable release directory for native CLI and verifier use.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_release_dir(root: impl AsRef<Path>) -> Result<Self, RacingCatalogResolutionError> {
        let root = root.as_ref();
        let manifest_bytes = read_file(root.join("catalog.json"))?;
        let release_identity_bytes = read_file(root.join("release.json"))?;
        let manifest: ResourceCatalogManifestV1 =
            parse_strict_document("catalog manifest", &manifest_bytes)?;
        let simulation_index_bytes =
            read_file(root.join(manifest.simulation_pack.index.path.as_str()))?;
        let presentation_index_bytes =
            read_file(root.join(manifest.presentation_pack.index.path.as_str()))?;
        let simulation_index: RacingSimulationIndexV1 =
            parse_strict_document("simulation index", &simulation_index_bytes)?;
        let mut resources = Vec::with_capacity(simulation_index.resources.len());
        for resource in &simulation_index.resources {
            resources.push((
                resource.path.to_string(),
                read_file(root.join(resource.path.as_str()))?,
            ));
        }
        Self::from_bytes(
            &manifest_bytes,
            &release_identity_bytes,
            &simulation_index_bytes,
            &presentation_index_bytes,
            resources,
        )
    }

    /// Returns the validated generic release manifest.
    #[must_use]
    pub const fn manifest(&self) -> &ResourceCatalogManifestV1 {
        &self.manifest
    }

    /// Returns the exact release identity verified from the manifest.
    #[must_use]
    pub const fn release_identity(&self) -> &CatalogReleaseIdentityV1 {
        &self.release_identity
    }

    /// Returns the validated Racing Simulation Pack index.
    #[must_use]
    pub const fn simulation_index(&self) -> &RacingSimulationIndexV1 {
        &self.simulation_index
    }

    /// Returns the validated Racing Presentation Pack index.
    #[must_use]
    pub const fn presentation_index(&self) -> &RacingPresentationIndexV1 {
        &self.presentation_index
    }

    /// Returns the optional immutable model-parameter resource selected by this release.
    ///
    /// Historical releases do not carry one and intentionally retain their
    /// compiled compatibility behavior.
    #[must_use]
    pub const fn model_parameters(&self) -> Option<&RacingModelParametersV1> {
        self.model_parameters.as_ref()
    }

    /// Binds validated resources to one exact deterministic run contract.
    ///
    /// The contract must select this release's Simulation Pack through its
    /// existing `data_pack` identity. `latest` and presentation metadata never
    /// become part of the logical run identity.
    pub fn resolve_scenario<Input>(
        &self,
        contract: DeterministicRunContractV1,
        input: Input,
    ) -> Result<ResolvedScenario<Input, RacingCatalogSnapshot>, RacingCatalogResolutionError> {
        ResolvedScenario::catalog_backed(
            contract,
            &self.manifest,
            self.release_identity.clone(),
            input,
            self.clone(),
        )
        .map_err(|error| RacingCatalogResolutionError::IncompatibleRun(error.to_string()))
    }

    pub(crate) fn resources(&self) -> impl Iterator<Item = (&CatalogPath, &[u8])> {
        self.resources
            .iter()
            .map(|(path, bytes)| (path, bytes.as_slice()))
    }
}

fn resolve_model_parameters(
    manifest: &ResourceCatalogManifestV1,
    simulation_index: &RacingSimulationIndexV1,
    supplied: &BTreeMap<CatalogPath, Vec<u8>>,
) -> Result<Option<RacingModelParametersV1>, RacingCatalogResolutionError> {
    let mut indexed = simulation_index.resources.iter().filter(|resource| {
        resource.id.as_str().starts_with(MODEL_PARAMETERS_ID_PREFIX)
            || resource
                .path
                .as_str()
                .starts_with(MODEL_PARAMETERS_PATH_PREFIX)
    });
    let Some(resource) = indexed.next() else {
        return Ok(None);
    };
    if indexed.next().is_some() {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "Racing catalog must select at most one model-parameter resource".to_string(),
        ));
    }

    let Some(stem) = resource
        .id
        .as_str()
        .strip_prefix(MODEL_PARAMETERS_ID_PREFIX)
    else {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            format!(
                "model-parameter path {} uses an incompatible resource ID {}",
                resource.path, resource.id
            ),
        ));
    };
    let expected_path = format!("{MODEL_PARAMETERS_PATH_PREFIX}{stem}.json");
    if resource.path.as_str() != expected_path {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            format!(
                "model-parameter resource {} must use path {expected_path}",
                resource.id
            ),
        ));
    }

    let bytes = supplied
        .get(&resource.path)
        .expect("indexed resources are checked before model-parameter resolution");
    let parameters: RacingModelParametersV1 = serde_json::from_slice(bytes).map_err(|error| {
        RacingCatalogResolutionError::InvalidResource {
            path: resource.path.clone(),
            reason: error.to_string(),
        }
    })?;
    parameters
        .validate()
        .map_err(|error| RacingCatalogResolutionError::InvalidResource {
            path: resource.path.clone(),
            reason: error.to_string(),
        })?;
    if parameters.identity.id != resource.id {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            format!(
                "model-parameter resource identity {} does not match index ID {}",
                parameters.identity.id, resource.id
            ),
        ));
    }

    let compatible_model = ArtifactIdentity {
        id: parameters.compatible_model.id.clone(),
        version: parameters.compatible_model.version.clone(),
        digest: Digest::from_bytes(
            format!(
                "{}:{}:catalog-compatibility",
                parameters.compatible_model.id, parameters.compatible_model.version
            )
            .as_bytes(),
        ),
    };
    parameters
        .validate_for_model(&compatible_model)
        .map_err(|error| {
            RacingCatalogResolutionError::InvalidResolvedResources(error.to_string())
        })?;
    manifest
        .compatibility
        .validate_for(&compatible_model, ContractVersion::V1)
        .map_err(|error| {
            RacingCatalogResolutionError::InvalidResolvedResources(format!(
                "model-parameter resource is incompatible with the catalog manifest: {error}"
            ))
        })?;

    Ok(Some(parameters))
}

fn validate_known_racing_model_compatibility(
    manifest: &ResourceCatalogManifestV1,
) -> Result<(), RacingCatalogResolutionError> {
    let supports_known_model = KNOWN_RACING_MODEL_VERSIONS.iter().any(|version| {
        let racing_model = ArtifactIdentity {
            id: "pitgun.racing"
                .parse()
                .expect("static Racing model identifier"),
            version: version.parse().expect("static Racing model version"),
            // Catalog compatibility is intentionally based on ID and version.
            // The executable model digest remains bound by the run contract.
            digest: Digest::from_bytes(
                format!("pitgun.racing:model-compatibility:{version}").as_bytes(),
            ),
        };
        manifest
            .compatibility
            .validate_for(&racing_model, ContractVersion::V1)
            .is_ok()
    });

    if supports_known_model {
        Ok(())
    } else {
        Err(RacingCatalogResolutionError::InvalidManifest(format!(
            "catalog does not support a known Racing model version ({})",
            KNOWN_RACING_MODEL_VERSIONS.join(", ")
        )))
    }
}

fn parse_strict_document<T>(
    document: &'static str,
    bytes: &[u8],
) -> Result<T, RacingCatalogResolutionError>
where
    T: DeserializeOwned,
{
    let text = std::str::from_utf8(bytes).map_err(|error| {
        RacingCatalogResolutionError::InvalidDocument {
            document,
            reason: error.to_string(),
        }
    })?;
    let canonical = canonicalize_json_str(text).map_err(|error| {
        RacingCatalogResolutionError::InvalidDocument {
            document,
            reason: error.to_string(),
        }
    })?;
    serde_json::from_slice(&canonical).map_err(|error| {
        RacingCatalogResolutionError::InvalidDocument {
            document,
            reason: error.to_string(),
        }
    })
}

fn verify_index_digest<T>(
    pack: &'static str,
    index: &T,
    expected: Digest,
) -> Result<(), RacingCatalogResolutionError>
where
    T: Serialize,
{
    let actual = canonical_json_digest(index).map_err(|error| {
        RacingCatalogResolutionError::InvalidDocument {
            document: "catalog index",
            reason: error.to_string(),
        }
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(RacingCatalogResolutionError::IndexDigestMismatch {
            pack,
            expected,
            actual,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, RacingCatalogResolutionError> {
    let path = path.as_ref();
    std::fs::read(path).map_err(|error| RacingCatalogResolutionError::FileRead {
        path: path.display().to_string(),
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::{
        EventOrderingV1, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
        RandomAlgorithm, RandomContractV1, ResourceCatalogManifestV1, RuntimeProfile,
        ScenarioIdentity, Seed, StreamDerivation, canonical_json_digest,
    };

    fn embedded_bundle() -> RacingCatalogBundleV1 {
        RacingCatalogBundleV1 {
            manifest: std::str::from_utf8(CATALOG_MANIFEST)
                .expect("manifest UTF-8")
                .to_owned(),
            release_identity: std::str::from_utf8(RELEASE_IDENTITY)
                .expect("release identity UTF-8")
                .to_owned(),
            simulation_index: std::str::from_utf8(SIMULATION_INDEX)
                .expect("simulation index UTF-8")
                .to_owned(),
            presentation_index: std::str::from_utf8(PRESENTATION_INDEX)
                .expect("presentation index UTF-8")
                .to_owned(),
            resources: EMBEDDED_FILES
                .iter()
                .map(|(path, contents)| RacingCatalogFileV1 {
                    path: format!("simulation/{path}"),
                    contents: std::str::from_utf8(contents)
                        .expect("Racing V1 resources are UTF-8 JSON")
                        .to_owned(),
                })
                .collect(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn filesystem_bundle(version: &str) -> RacingCatalogBundleV1 {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../catalogs/racing")
            .join(format!("v{version}"));
        let manifest = std::fs::read_to_string(root.join("catalog.json")).expect("manifest");
        let release_identity =
            std::fs::read_to_string(root.join("release.json")).expect("release identity");
        let simulation_index =
            std::fs::read_to_string(root.join("simulation/index.json")).expect("simulation index");
        let presentation_index = std::fs::read_to_string(root.join("presentation/index.json"))
            .expect("presentation index");
        let index: RacingSimulationIndexV1 =
            serde_json::from_str(&simulation_index).expect("simulation index JSON");
        let resources = index
            .resources
            .iter()
            .map(|resource| RacingCatalogFileV1 {
                path: resource.path.to_string(),
                contents: std::fs::read_to_string(root.join(resource.path.as_str()))
                    .expect("simulation resource"),
            })
            .collect();
        RacingCatalogBundleV1 {
            manifest,
            release_identity,
            simulation_index,
            presentation_index,
            resources,
        }
    }

    fn resign_bundle(bundle: &mut RacingCatalogBundleV1) {
        let mut index: RacingSimulationIndexV1 =
            serde_json::from_str(&bundle.simulation_index).expect("simulation index JSON");
        for resource in &mut index.resources {
            let file = bundle
                .resources
                .iter()
                .find(|file| file.path == resource.path.as_str())
                .expect("indexed resource");
            resource.digest = Digest::from_bytes(file.contents.as_bytes());
        }
        bundle.simulation_index = serde_json::to_string(&index).expect("simulation index JSON");
        let simulation_digest = canonical_json_digest(&index).expect("simulation index digest");

        let mut manifest: ResourceCatalogManifestV1 =
            serde_json::from_str(&bundle.manifest).expect("manifest JSON");
        manifest.simulation_pack.identity.digest = simulation_digest;
        manifest.simulation_pack.index.digest = simulation_digest;
        bundle.manifest = serde_json::to_string(&manifest).expect("manifest JSON");

        let mut release: CatalogReleaseIdentityV1 =
            serde_json::from_str(&bundle.release_identity).expect("release identity JSON");
        release.manifest_digest = canonical_json_digest(&manifest).expect("manifest digest");
        bundle.release_identity = serde_json::to_string(&release).expect("release identity JSON");
    }

    fn compatible_contract(snapshot: &RacingCatalogSnapshot) -> DeterministicRunContractV1 {
        DeterministicRunContractV1 {
            contract_version: ContractVersion::V1,
            scenario: ScenarioIdentity {
                id: "racing.weekend".parse().expect("scenario id"),
                version: "1.0.0".parse().expect("scenario version"),
            },
            model: ArtifactIdentity {
                id: "pitgun.racing".parse().expect("model id"),
                version: "1.0.0".parse().expect("model version"),
                digest: Digest::from_bytes(b"model"),
            },
            data_pack: snapshot.manifest().simulation_pack.identity.clone(),
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
                digest: Digest::from_bytes(b"input"),
            },
        }
    }

    #[test]
    fn embedded_release_passes_the_complete_resolution_boundary() {
        let snapshot = RacingCatalogSnapshot::embedded().expect("embedded catalog");

        assert_eq!(snapshot.manifest().catalog.id.to_string(), "pitgun.racing");
        assert_eq!(snapshot.manifest().catalog.version.to_string(), "1.0.0");
        assert_eq!(
            snapshot.resources().count(),
            snapshot.simulation_index().resources.len()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn filesystem_and_embedded_adapters_resolve_identically() {
        let embedded = RacingCatalogSnapshot::embedded().expect("embedded catalog");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.0.0");
        let filesystem = RacingCatalogSnapshot::from_release_dir(root).expect("filesystem catalog");

        assert_eq!(filesystem.manifest(), embedded.manifest());
        assert_eq!(filesystem.release_identity(), embedded.release_identity());
        assert_eq!(filesystem.simulation_index(), embedded.simulation_index());
        assert_eq!(
            filesystem.presentation_index(),
            embedded.presentation_index()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn policy_catalog_release_resolves_without_changing_the_embedded_fallback() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.1.0");
        let snapshot =
            RacingCatalogSnapshot::from_release_dir(root).expect("policy catalog release");

        assert_eq!(snapshot.manifest().catalog.version.to_string(), "1.1.0");
        assert!(
            snapshot
                .simulation_index()
                .resources
                .iter()
                .any(|resource| { resource.path.as_str() == "simulation/policies/reference.json" })
        );
        assert_eq!(
            RacingCatalogSnapshot::embedded()
                .expect("embedded fallback")
                .manifest()
                .catalog
                .version
                .to_string(),
            "1.0.0"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn model_v2_catalog_release_resolves_without_changing_the_embedded_fallback() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.2.0");
        let snapshot = RacingCatalogSnapshot::from_release_dir(root).expect("model V2 catalog");

        assert_eq!(snapshot.manifest().catalog.version.to_string(), "1.2.0");
        let compatible_model = &snapshot.manifest().compatibility.models[0];
        assert_eq!(compatible_model.id.to_string(), "pitgun.racing");
        assert_eq!(
            compatible_model
                .versions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["2.0.0"]
        );
        assert_eq!(
            RacingCatalogSnapshot::embedded()
                .expect("embedded fallback")
                .manifest()
                .catalog
                .version
                .to_string(),
            "1.0.0"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn competitive_policy_v2_release_resolves_and_keeps_late_eras_disabled() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.3.0");
        let snapshot =
            RacingCatalogSnapshot::from_release_dir(root).expect("competitive policy catalog");

        assert_eq!(snapshot.manifest().catalog.version.to_string(), "1.3.0");
        assert!(
            snapshot
                .simulation_index()
                .resources
                .iter()
                .any(|resource| {
                    resource.path.as_str() == "simulation/policies/competitive.json"
                })
        );
        let policy_bytes = snapshot
            .resources()
            .find_map(|(path, bytes)| {
                (path.as_str() == "simulation/policies/competitive.json").then_some(bytes)
            })
            .expect("competitive policy resource");
        let policy: serde_json::Value =
            serde_json::from_slice(policy_bytes).expect("competitive policy JSON");
        assert_eq!(
            policy["scope"]["supported_game_eras"],
            serde_json::json!([1, 2, 3, 4, 5])
        );
        assert_eq!(
            policy["scope"]["unsupported_game_eras"],
            serde_json::json!([6, 7])
        );
        assert_eq!(
            policy["strategy"]["player_strategy_influence_allowed"],
            serde_json::json!(false)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn model_parameter_release_resolves_without_mutating_historical_catalogs() {
        let historical_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.3.0");
        let historical = RacingCatalogSnapshot::from_release_dir(historical_root)
            .expect("historical catalog release");
        assert!(historical.model_parameters().is_none());

        let resource_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.4.0");
        let snapshot = RacingCatalogSnapshot::from_release_dir(resource_root)
            .expect("model-parameter catalog release");
        let parameters = snapshot
            .model_parameters()
            .expect("catalog-backed model parameters");

        assert_eq!(snapshot.manifest().catalog.version.to_string(), "1.4.0");
        assert_eq!(
            parameters.identity.id.as_str(),
            "pitgun.racing.model-parameters.v2-compatibility"
        );
        assert_eq!(
            parameters
                .canonical_digest()
                .expect("parameter digest")
                .to_string(),
            "sha256:1c60391e5c536248153b5cae8608bc126f85b5ca31fe04b5cc84a424673e3f50"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn model_parameter_release_rejects_missing_or_tampered_bytes() {
        let mut missing = filesystem_bundle("1.4.0");
        missing
            .resources
            .retain(|file| file.path != "simulation/model-parameters/v2-compatibility.json");
        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(missing),
            Err(RacingCatalogResolutionError::MissingResource(path))
                if path.as_str() == "simulation/model-parameters/v2-compatibility.json"
        ));

        let mut tampered = filesystem_bundle("1.4.0");
        tampered
            .resources
            .iter_mut()
            .find(|file| file.path == "simulation/model-parameters/v2-compatibility.json")
            .expect("model parameters")
            .contents
            .push(' ');
        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(tampered),
            Err(RacingCatalogResolutionError::ResourceDigestMismatch { path, .. })
                if path.as_str() == "simulation/model-parameters/v2-compatibility.json"
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn model_parameter_release_rejects_malformed_or_incompatible_resources() {
        let mut malformed = filesystem_bundle("1.4.0");
        malformed
            .resources
            .iter_mut()
            .find(|file| file.path == "simulation/model-parameters/v2-compatibility.json")
            .expect("model parameters")
            .contents = "{}".to_string();
        resign_bundle(&mut malformed);
        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(malformed),
            Err(RacingCatalogResolutionError::InvalidResource { path, .. })
                if path.as_str() == "simulation/model-parameters/v2-compatibility.json"
        ));

        let mut incompatible = filesystem_bundle("1.4.0");
        let file = incompatible
            .resources
            .iter_mut()
            .find(|file| file.path == "simulation/model-parameters/v2-compatibility.json")
            .expect("model parameters");
        let mut value: serde_json::Value =
            serde_json::from_str(&file.contents).expect("model parameters JSON");
        value["compatible_model"]["version"] = serde_json::json!("1.0.0");
        file.contents = serde_json::to_string(&value).expect("model parameters JSON");
        resign_bundle(&mut incompatible);
        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(incompatible),
            Err(RacingCatalogResolutionError::InvalidResource { path, .. })
                if path.as_str() == "simulation/model-parameters/v2-compatibility.json"
        ));
    }

    #[test]
    fn embedded_model_generations_are_mutually_incompatible() {
        let model_v1_catalog = RacingCatalogSnapshot::embedded().expect("model V1 catalog");
        let model_v2_catalog =
            RacingCatalogSnapshot::embedded_model_v2().expect("model V2 catalog");

        assert!(
            model_v1_catalog
                .manifest()
                .compatibility
                .validate_for(&crate::racing_model_v2_identity(), ContractVersion::V1)
                .is_err()
        );
        assert!(
            model_v2_catalog
                .manifest()
                .compatibility
                .validate_for(&crate::racing_model_v1_identity(), ContractVersion::V1)
                .is_err()
        );
    }

    #[test]
    fn catalog_compatible_only_with_an_unknown_model_version_fails_closed() {
        let mut bundle = embedded_bundle();
        let mut manifest: ResourceCatalogManifestV1 =
            serde_json::from_str(&bundle.manifest).expect("manifest JSON");
        manifest.compatibility.models[0].versions =
            ["9.0.0".parse().expect("model version")].into();
        bundle.manifest = serde_json::to_string(&manifest).expect("manifest JSON");

        let error = RacingCatalogSnapshot::from_bundle(bundle)
            .expect_err("unknown model compatibility must fail");
        assert!(matches!(
            error,
            RacingCatalogResolutionError::InvalidManifest(reason)
                if reason.contains("known Racing model version")
        ));
    }

    #[test]
    fn browser_bundle_and_embedded_adapters_resolve_identically() {
        let embedded = RacingCatalogSnapshot::embedded().expect("embedded catalog");
        let json = serde_json::to_string(&embedded_bundle()).expect("bundle JSON");
        let browser = RacingCatalogSnapshot::from_bundle_json(&json).expect("browser catalog");

        assert_eq!(browser.manifest(), embedded.manifest());
        assert_eq!(browser.release_identity(), embedded.release_identity());
        assert_eq!(browser.simulation_index(), embedded.simulation_index());
        assert_eq!(browser.presentation_index(), embedded.presentation_index());
    }

    #[test]
    fn resolved_scenario_binds_the_exact_simulation_pack() {
        let snapshot = RacingCatalogSnapshot::embedded().expect("embedded catalog");
        let contract = compatible_contract(&snapshot);
        let resolved = snapshot
            .resolve_scenario(contract.clone(), "input")
            .expect("compatible run");

        assert_eq!(resolved.contract(), &contract);
        assert_eq!(
            resolved.catalog_release(),
            Some(snapshot.release_identity())
        );

        let mut incompatible = contract;
        incompatible.data_pack.digest = Digest::from_bytes(b"other pack");
        let error = snapshot
            .resolve_scenario(incompatible, "input")
            .expect_err("different Simulation Pack must fail");
        assert!(matches!(
            error,
            RacingCatalogResolutionError::IncompatibleRun(_)
        ));
    }

    #[test]
    fn resource_digest_mismatch_fails_before_resolution() {
        let mut resources = EMBEDDED_FILES
            .iter()
            .map(|(path, bytes)| (format!("simulation/{path}"), bytes.to_vec()))
            .collect::<Vec<_>>();
        resources[0].1.push(b' ');

        let error = RacingCatalogSnapshot::from_bytes(
            CATALOG_MANIFEST,
            RELEASE_IDENTITY,
            SIMULATION_INDEX,
            PRESENTATION_INDEX,
            resources,
        )
        .expect_err("mutated resource must fail");

        assert!(matches!(
            error,
            RacingCatalogResolutionError::ResourceDigestMismatch { .. }
        ));
    }

    #[test]
    fn missing_resource_fails_closed() {
        let resources = EMBEDDED_FILES
            .iter()
            .skip(1)
            .map(|(path, bytes)| (format!("simulation/{path}"), bytes.to_vec()));

        let error = RacingCatalogSnapshot::from_bytes(
            CATALOG_MANIFEST,
            RELEASE_IDENTITY,
            SIMULATION_INDEX,
            PRESENTATION_INDEX,
            resources,
        )
        .expect_err("missing resource must fail");

        assert!(matches!(
            error,
            RacingCatalogResolutionError::MissingResource(_)
        ));
    }
}

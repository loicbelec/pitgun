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
    ComponentCapabilityProfileV1, RACING_DRIVER_INSTRUCTION_PROFILE_ID,
    RACING_DRIVER_INSTRUCTION_PROFILE_VERSION, RacingDriverInstructionProfileV1,
    RacingModelParametersV1, RacingPresentationIndexV1, RacingSimulationIndexV1,
    VehicleComponentKind,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{
    EMBEDDED_FILES, MODEL_V2_EMBEDDED_FILES, MODEL_V3_COMPONENT_EMBEDDED_FILES,
    MODEL_V3_THERMAL_EMBEDDED_FILES, PRESENTATION_INDEX, V3PowerUnitThermalProfileCandidateV2,
    V3ThermalFamilyProfileCandidateV1, racing_model_v3_component_candidate_identity,
    racing_model_v3_thermal_candidate_identity,
};

const KNOWN_RACING_MODELS: [(&str, &str); 4] = [
    ("pitgun.racing", "1.0.0"),
    ("pitgun.racing", "2.0.0"),
    ("pitgun.racing-v3-candidate", "0.10.0"),
    ("pitgun.racing-v3-candidate", "0.11.0"),
];
const MODEL_PARAMETERS_ID_PREFIX: &str = "pitgun.racing.model-parameters.";
const MODEL_PARAMETERS_PATH_PREFIX: &str = "simulation/model-parameters/";
const THERMAL_FAMILY_RESOURCE_ID: &str = "pitgun.racing.thermal-profiles.family-v1";
const THERMAL_FAMILY_PROFILE_PATH: &str = "simulation/thermal-profiles/family-v1.json";
const POWER_UNIT_THERMAL_RESOURCE_ID: &str = "pitgun.racing.thermal-profiles.family-v2";
const POWER_UNIT_THERMAL_PROFILE_PATH: &str = "simulation/thermal-profiles/family-v2.json";
const COMPONENT_CAPABILITY_RESOURCE_ID: &str = "pitgun.racing.component-capabilities.v1";
const COMPONENT_CAPABILITY_PROFILE_PATH: &str = "simulation/component-capabilities/v1.json";
const DRIVER_INSTRUCTION_PROFILE_RESOURCE_ID: &str = "pitgun.racing.driver-instructions";
const DRIVER_INSTRUCTION_PROFILE_PATH: &str = "simulation/driver-instructions/profile-v1.json";

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
const MODEL_V3_THERMAL_CATALOG_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.5.0/catalog.json"
));
const MODEL_V3_THERMAL_RELEASE_IDENTITY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.5.0/release.json"
));
const MODEL_V3_THERMAL_SIMULATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.5.0/simulation/index.json"
));
const MODEL_V3_THERMAL_PRESENTATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.5.0/presentation/index.json"
));
const MODEL_V3_COMPONENT_CATALOG_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.6.0/catalog.json"
));
const MODEL_V3_COMPONENT_RELEASE_IDENTITY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.6.0/release.json"
));
const MODEL_V3_COMPONENT_SIMULATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.6.0/simulation/index.json"
));
const MODEL_V3_COMPONENT_PRESENTATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.6.0/presentation/index.json"
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
    model_parameters_identity: Option<ArtifactIdentity>,
    thermal_family_profile: Option<V3ThermalFamilyProfileCandidateV1>,
    thermal_family_profile_identity: Option<ArtifactIdentity>,
    power_unit_thermal_profile: Option<V3PowerUnitThermalProfileCandidateV2>,
    power_unit_thermal_profile_identity: Option<ArtifactIdentity>,
    component_capability_profile: Option<ComponentCapabilityProfileV1>,
    component_capability_profile_identity: Option<ArtifactIdentity>,
    driver_instruction_profile: Option<RacingDriverInstructionProfileV1>,
    driver_instruction_profile_identity: Option<ArtifactIdentity>,
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

        let resolved_model_parameters =
            resolve_model_parameters(&manifest, &simulation_index, &supplied)?;
        let (model_parameters, model_parameters_identity) = resolved_model_parameters
            .map_or((None, None), |(parameters, identity)| {
                (Some(parameters), Some(identity))
            });
        let resolved_thermal_family_profile =
            resolve_thermal_family_profile(&manifest, &simulation_index, &supplied)?;
        let (thermal_family_profile, thermal_family_profile_identity) =
            resolved_thermal_family_profile.map_or((None, None), |(profile, identity)| {
                (Some(profile), Some(identity))
            });
        let resolved_power_unit_thermal_profile =
            resolve_power_unit_thermal_profile(&manifest, &simulation_index, &supplied)?;
        let (power_unit_thermal_profile, power_unit_thermal_profile_identity) =
            resolved_power_unit_thermal_profile.map_or((None, None), |(profile, identity)| {
                (Some(profile), Some(identity))
            });
        let resolved_component_capability_profile =
            resolve_component_capability_profile(&manifest, &simulation_index, &supplied)?;
        let (component_capability_profile, component_capability_profile_identity) =
            resolved_component_capability_profile.map_or((None, None), |(profile, identity)| {
                (Some(profile), Some(identity))
            });
        let resolved_driver_instruction_profile =
            resolve_driver_instruction_profile(&simulation_index, &supplied)?;
        let (driver_instruction_profile, driver_instruction_profile_identity) =
            resolved_driver_instruction_profile.map_or((None, None), |(profile, identity)| {
                (Some(profile), Some(identity))
            });

        let snapshot = Self {
            manifest,
            release_identity,
            simulation_index,
            presentation_index,
            resources: supplied,
            model_parameters,
            model_parameters_identity,
            thermal_family_profile,
            thermal_family_profile_identity,
            power_unit_thermal_profile,
            power_unit_thermal_profile_identity,
            component_capability_profile,
            component_capability_profile_identity,
            driver_instruction_profile,
            driver_instruction_profile_identity,
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

    /// Resolves the immutable non-production catalog for the reviewed V3 thermal candidate.
    ///
    /// This leaves the public `LATEST` pointer and both published model fallbacks unchanged.
    pub fn embedded_model_v3_thermal() -> Result<Self, RacingCatalogResolutionError> {
        let resources = MODEL_V3_THERMAL_EMBEDDED_FILES
            .iter()
            .map(|(path, bytes)| (format!("simulation/{path}"), bytes.to_vec()));
        Self::from_bytes(
            MODEL_V3_THERMAL_CATALOG_MANIFEST,
            MODEL_V3_THERMAL_RELEASE_IDENTITY,
            MODEL_V3_THERMAL_SIMULATION_INDEX,
            MODEL_V3_THERMAL_PRESENTATION_INDEX,
            resources,
        )
    }

    /// Resolves the immutable component-composed Model V3 candidate catalog.
    pub fn embedded_model_v3_component() -> Result<Self, RacingCatalogResolutionError> {
        let resources = MODEL_V3_COMPONENT_EMBEDDED_FILES
            .iter()
            .map(|(path, bytes)| (format!("simulation/{path}"), bytes.to_vec()));
        Self::from_bytes(
            MODEL_V3_COMPONENT_CATALOG_MANIFEST,
            MODEL_V3_COMPONENT_RELEASE_IDENTITY,
            MODEL_V3_COMPONENT_SIMULATION_INDEX,
            MODEL_V3_COMPONENT_PRESENTATION_INDEX,
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

    /// Returns the exact semantic and content identity of the selected parameters.
    ///
    /// The digest covers the exact resource bytes validated against the immutable
    /// Simulation Pack index. Historical releases intentionally return `None`.
    #[must_use]
    pub const fn model_parameters_identity(&self) -> Option<&ArtifactIdentity> {
        self.model_parameters_identity.as_ref()
    }

    /// Returns the reviewed thermal-family profile selected by this release.
    #[must_use]
    pub const fn thermal_family_profile(&self) -> Option<&V3ThermalFamilyProfileCandidateV1> {
        self.thermal_family_profile.as_ref()
    }

    /// Returns the exact semantic and byte identity of the thermal-family profile.
    #[must_use]
    pub const fn thermal_family_profile_identity(&self) -> Option<&ArtifactIdentity> {
        self.thermal_family_profile_identity.as_ref()
    }

    /// Returns the reviewed thermal profile resolved by installed power unit.
    #[must_use]
    pub const fn power_unit_thermal_profile(
        &self,
    ) -> Option<&V3PowerUnitThermalProfileCandidateV2> {
        self.power_unit_thermal_profile.as_ref()
    }

    /// Returns the exact identity of the installed-power-unit thermal profile.
    #[must_use]
    pub const fn power_unit_thermal_profile_identity(&self) -> Option<&ArtifactIdentity> {
        self.power_unit_thermal_profile_identity.as_ref()
    }

    /// Returns the versioned component-to-capability mapping selected by this release.
    #[must_use]
    pub const fn component_capability_profile(&self) -> Option<&ComponentCapabilityProfileV1> {
        self.component_capability_profile.as_ref()
    }

    /// Returns the exact byte identity of the component-capability mapping.
    #[must_use]
    pub const fn component_capability_profile_identity(&self) -> Option<&ArtifactIdentity> {
        self.component_capability_profile_identity.as_ref()
    }

    /// Returns the optional immutable profile governing live driver instructions.
    #[must_use]
    pub const fn driver_instruction_profile(&self) -> Option<&RacingDriverInstructionProfileV1> {
        self.driver_instruction_profile.as_ref()
    }

    /// Returns the exact byte identity of the selected driver-instruction profile.
    #[must_use]
    pub const fn driver_instruction_profile_identity(&self) -> Option<&ArtifactIdentity> {
        self.driver_instruction_profile_identity.as_ref()
    }

    /// Recreates the transport-neutral bundle for this validated snapshot.
    ///
    /// This is primarily useful to exercise the same byte-validation boundary in
    /// native and WASM tests without adding filesystem or network concerns.
    pub fn to_bundle(&self) -> Result<RacingCatalogBundleV1, serde_json::Error> {
        Ok(RacingCatalogBundleV1 {
            manifest: serde_json::to_string(&self.manifest)?,
            release_identity: serde_json::to_string(&self.release_identity)?,
            simulation_index: serde_json::to_string(&self.simulation_index)?,
            presentation_index: serde_json::to_string(&self.presentation_index)?,
            resources: self
                .resources
                .iter()
                .map(|(path, bytes)| RacingCatalogFileV1 {
                    path: path.to_string(),
                    contents: String::from_utf8(bytes.clone())
                        .expect("validated Racing resources are UTF-8 JSON"),
                })
                .collect(),
        })
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
) -> Result<Option<(RacingModelParametersV1, ArtifactIdentity)>, RacingCatalogResolutionError> {
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

    let identity = ArtifactIdentity {
        id: parameters.identity.id.clone(),
        version: parameters.identity.version.clone(),
        digest: resource.digest,
    };

    Ok(Some((parameters, identity)))
}

fn resolve_thermal_family_profile(
    manifest: &ResourceCatalogManifestV1,
    simulation_index: &RacingSimulationIndexV1,
    supplied: &BTreeMap<CatalogPath, Vec<u8>>,
) -> Result<
    Option<(V3ThermalFamilyProfileCandidateV1, ArtifactIdentity)>,
    RacingCatalogResolutionError,
> {
    let model = racing_model_v3_thermal_candidate_identity();
    let supports_v3_thermal = manifest
        .compatibility
        .validate_for(&model, ContractVersion::V1)
        .is_ok();
    let mut indexed = simulation_index.resources.iter().filter(|resource| {
        resource.id.as_str() == THERMAL_FAMILY_RESOURCE_ID
            || resource.path.as_str() == THERMAL_FAMILY_PROFILE_PATH
    });
    let Some(resource) = indexed.next() else {
        if supports_v3_thermal {
            return Err(RacingCatalogResolutionError::InvalidResolvedResources(
                "Model V3 thermal catalog is missing its thermal-family profile".to_string(),
            ));
        }
        return Ok(None);
    };
    if indexed.next().is_some() {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "Racing catalog must select exactly one thermal-family profile".to_string(),
        ));
    }
    if !supports_v3_thermal {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "thermal-family profile requires the exact Model V3 thermal compatibility".to_string(),
        ));
    }
    if resource.id.as_str() != THERMAL_FAMILY_RESOURCE_ID
        || resource.path.as_str() != THERMAL_FAMILY_PROFILE_PATH
    {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            format!(
                "thermal-family profile must use resource ID {THERMAL_FAMILY_RESOURCE_ID} and path {THERMAL_FAMILY_PROFILE_PATH}"
            ),
        ));
    }

    let bytes = supplied
        .get(&resource.path)
        .expect("indexed resources are checked before thermal-profile resolution");
    let text = std::str::from_utf8(bytes).map_err(|error| {
        RacingCatalogResolutionError::InvalidResource {
            path: resource.path.clone(),
            reason: error.to_string(),
        }
    })?;
    let profile = V3ThermalFamilyProfileCandidateV1::from_exact_json(text).map_err(|reason| {
        RacingCatalogResolutionError::InvalidResource {
            path: resource.path.clone(),
            reason,
        }
    })?;
    let identity = profile.identity();
    if identity.digest != resource.digest {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "thermal-family profile identity does not match its indexed bytes".to_string(),
        ));
    }

    Ok(Some((profile, identity)))
}

fn resolve_power_unit_thermal_profile(
    manifest: &ResourceCatalogManifestV1,
    simulation_index: &RacingSimulationIndexV1,
    supplied: &BTreeMap<CatalogPath, Vec<u8>>,
) -> Result<
    Option<(V3PowerUnitThermalProfileCandidateV2, ArtifactIdentity)>,
    RacingCatalogResolutionError,
> {
    let model = racing_model_v3_component_candidate_identity();
    let supports_component_model = manifest
        .compatibility
        .validate_for(&model, ContractVersion::V1)
        .is_ok();
    let mut indexed = simulation_index.resources.iter().filter(|resource| {
        resource.id.as_str() == POWER_UNIT_THERMAL_RESOURCE_ID
            || resource.path.as_str() == POWER_UNIT_THERMAL_PROFILE_PATH
    });
    let Some(resource) = indexed.next() else {
        if supports_component_model {
            return Err(RacingCatalogResolutionError::InvalidResolvedResources(
                "component-composed Model V3 catalog is missing its power-unit thermal profile"
                    .to_string(),
            ));
        }
        return Ok(None);
    };
    if indexed.next().is_some() {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "Racing catalog must select exactly one power-unit thermal profile".to_string(),
        ));
    }
    if !supports_component_model {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "power-unit thermal profile requires exact component-composed Model V3 compatibility"
                .to_string(),
        ));
    }
    if resource.id.as_str() != POWER_UNIT_THERMAL_RESOURCE_ID
        || resource.path.as_str() != POWER_UNIT_THERMAL_PROFILE_PATH
    {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            format!(
                "power-unit thermal profile must use resource ID {POWER_UNIT_THERMAL_RESOURCE_ID} and path {POWER_UNIT_THERMAL_PROFILE_PATH}"
            ),
        ));
    }
    let bytes = supplied
        .get(&resource.path)
        .expect("indexed resources are checked before power-unit thermal resolution");
    let text = std::str::from_utf8(bytes).map_err(|error| {
        RacingCatalogResolutionError::InvalidResource {
            path: resource.path.clone(),
            reason: error.to_string(),
        }
    })?;
    let profile =
        V3PowerUnitThermalProfileCandidateV2::from_exact_json(text).map_err(|reason| {
            RacingCatalogResolutionError::InvalidResource {
                path: resource.path.clone(),
                reason,
            }
        })?;
    let identity = profile.identity();
    if identity.digest != resource.digest {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "power-unit thermal profile identity does not match its indexed bytes".to_string(),
        ));
    }
    Ok(Some((profile, identity)))
}

fn resolve_component_capability_profile(
    manifest: &ResourceCatalogManifestV1,
    simulation_index: &RacingSimulationIndexV1,
    supplied: &BTreeMap<CatalogPath, Vec<u8>>,
) -> Result<Option<(ComponentCapabilityProfileV1, ArtifactIdentity)>, RacingCatalogResolutionError>
{
    let model = racing_model_v3_component_candidate_identity();
    let supports_component_model = manifest
        .compatibility
        .validate_for(&model, ContractVersion::V1)
        .is_ok();
    let mut indexed = simulation_index.resources.iter().filter(|resource| {
        resource.id.as_str() == COMPONENT_CAPABILITY_RESOURCE_ID
            || resource.path.as_str() == COMPONENT_CAPABILITY_PROFILE_PATH
    });
    let Some(resource) = indexed.next() else {
        if supports_component_model {
            return Err(RacingCatalogResolutionError::InvalidResolvedResources(
                "component-composed Model V3 catalog is missing its capability profile".to_string(),
            ));
        }
        return Ok(None);
    };
    if indexed.next().is_some() {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "Racing catalog must select exactly one component-capability profile".to_string(),
        ));
    }
    if !supports_component_model {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "component-capability profile requires exact component-composed Model V3 compatibility"
                .to_string(),
        ));
    }
    if resource.id.as_str() != COMPONENT_CAPABILITY_RESOURCE_ID
        || resource.path.as_str() != COMPONENT_CAPABILITY_PROFILE_PATH
    {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            format!(
                "component-capability profile must use resource ID {COMPONENT_CAPABILITY_RESOURCE_ID} and path {COMPONENT_CAPABILITY_PROFILE_PATH}"
            ),
        ));
    }
    let bytes = supplied
        .get(&resource.path)
        .expect("indexed resources are checked before capability resolution");
    let profile: ComponentCapabilityProfileV1 = serde_json::from_slice(bytes).map_err(|error| {
        RacingCatalogResolutionError::InvalidResource {
            path: resource.path.clone(),
            reason: error.to_string(),
        }
    })?;
    let mut expected = BTreeMap::<VehicleComponentKind, BTreeSet<String>>::new();
    for indexed_resource in &simulation_index.resources {
        let path = indexed_resource.path.as_str();
        let (kind, prefix) = if path.starts_with("simulation/aero/") {
            (VehicleComponentKind::AerodynamicPackage, "simulation/aero/")
        } else if path.starts_with("simulation/chassis/") {
            (VehicleComponentKind::Chassis, "simulation/chassis/")
        } else if path.starts_with("simulation/engines/") {
            (VehicleComponentKind::PowerUnit, "simulation/engines/")
        } else if path.starts_with("simulation/tires/") {
            (VehicleComponentKind::TireSpecification, "simulation/tires/")
        } else {
            continue;
        };
        let component_id = path
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(".json"))
            .ok_or_else(|| {
                RacingCatalogResolutionError::InvalidResolvedResources(format!(
                    "invalid component resource path {path}"
                ))
            })?;
        expected
            .entry(kind)
            .or_default()
            .insert(component_id.to_string());
    }
    profile.validate_against(&expected).map_err(|error| {
        RacingCatalogResolutionError::InvalidResolvedResources(error.to_string())
    })?;
    let identity = ArtifactIdentity {
        id: resource.id.clone(),
        version: manifest.catalog.version.clone(),
        digest: resource.digest,
    };
    Ok(Some((profile, identity)))
}

fn resolve_driver_instruction_profile(
    simulation_index: &RacingSimulationIndexV1,
    supplied: &BTreeMap<CatalogPath, Vec<u8>>,
) -> Result<
    Option<(RacingDriverInstructionProfileV1, ArtifactIdentity)>,
    RacingCatalogResolutionError,
> {
    let mut indexed = simulation_index.resources.iter().filter(|resource| {
        resource.id.as_str() == DRIVER_INSTRUCTION_PROFILE_RESOURCE_ID
            || resource.path.as_str() == DRIVER_INSTRUCTION_PROFILE_PATH
    });
    let Some(resource) = indexed.next() else {
        return Ok(None);
    };
    if indexed.next().is_some() {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            "Racing catalog must select at most one driver-instruction profile".to_string(),
        ));
    }
    if resource.id.as_str() != DRIVER_INSTRUCTION_PROFILE_RESOURCE_ID
        || resource.path.as_str() != DRIVER_INSTRUCTION_PROFILE_PATH
    {
        return Err(RacingCatalogResolutionError::InvalidResolvedResources(
            format!(
                "driver-instruction profile must use catalog resource ID {DRIVER_INSTRUCTION_PROFILE_RESOURCE_ID} and path {DRIVER_INSTRUCTION_PROFILE_PATH}"
            ),
        ));
    }
    let bytes = supplied
        .get(&resource.path)
        .expect("indexed resources are checked before instruction-profile resolution");
    let profile: RacingDriverInstructionProfileV1 =
        serde_json::from_slice(bytes).map_err(|error| {
            RacingCatalogResolutionError::InvalidResource {
                path: resource.path.clone(),
                reason: error.to_string(),
            }
        })?;
    profile.validate().map_err(|error| {
        RacingCatalogResolutionError::InvalidResolvedResources(error.to_string())
    })?;
    let identity = ArtifactIdentity {
        id: RACING_DRIVER_INSTRUCTION_PROFILE_ID
            .parse()
            .expect("static driver-instruction profile identity"),
        version: RACING_DRIVER_INSTRUCTION_PROFILE_VERSION
            .parse()
            .expect("static driver-instruction profile version"),
        digest: resource.digest,
    };
    Ok(Some((profile, identity)))
}

fn validate_known_racing_model_compatibility(
    manifest: &ResourceCatalogManifestV1,
) -> Result<(), RacingCatalogResolutionError> {
    let supports_known_model = KNOWN_RACING_MODELS.iter().any(|(id, version)| {
        let racing_model = ArtifactIdentity {
            id: id.parse().expect("static Racing model identifier"),
            version: version.parse().expect("static Racing model version"),
            // Catalog compatibility is intentionally based on ID and version.
            // The executable model digest remains bound by the run contract.
            digest: Digest::from_bytes(format!("{id}:model-compatibility:{version}").as_bytes()),
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
            "catalog does not support a known Racing model ({})",
            KNOWN_RACING_MODELS
                .iter()
                .map(|(id, version)| format!("{id}@{version}"))
                .collect::<Vec<_>>()
                .join(", ")
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
        CatalogResourceV1, EventOrderingV1, InputCanonicalization, InputIdentity, InputMediaType,
        LogicalClockV1, RandomAlgorithm, RandomContractV1, ResourceCatalogManifestV1,
        RuntimeProfile, ScenarioIdentity, Seed, StreamDerivation, canonical_json_digest,
    };
    use pitgun_racing_contract::{
        RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionProfileVersion,
        RacingDrivingMode,
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

    fn bundle_with_driver_instruction_profile() -> RacingCatalogBundleV1 {
        let mut bundle = embedded_bundle();
        let profile = RacingDriverInstructionProfileV1 {
            schema_version: RacingDriverInstructionProfileVersion::V1,
            default_mode: RacingDrivingMode::Balanced,
            boundary_granularity: RacingDriverInstructionBoundaryGranularityV1::LapStart,
            max_events_per_session: 64,
        };
        let contents = serde_json::to_string(&profile).expect("instruction profile JSON");
        let mut index: RacingSimulationIndexV1 =
            serde_json::from_str(&bundle.simulation_index).expect("simulation index JSON");
        index.resources.push(CatalogResourceV1 {
            id: DRIVER_INSTRUCTION_PROFILE_RESOURCE_ID
                .parse()
                .expect("profile resource id"),
            path: DRIVER_INSTRUCTION_PROFILE_PATH
                .parse()
                .expect("profile resource path"),
            media_type: "application/json".parse().expect("JSON media type"),
            digest: Digest::from_bytes(contents.as_bytes()),
        });
        index
            .resources
            .sort_by(|left, right| left.id.cmp(&right.id));
        bundle.simulation_index = serde_json::to_string(&index).expect("simulation index JSON");
        bundle.resources.push(RacingCatalogFileV1 {
            path: DRIVER_INSTRUCTION_PROFILE_PATH.to_string(),
            contents,
        });
        resign_bundle(&mut bundle);
        bundle
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
        assert!(historical.model_parameters_identity().is_none());

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
        let identity = snapshot
            .model_parameters_identity()
            .expect("catalog-backed parameter identity");
        assert_eq!(identity.id, parameters.identity.id);
        assert_eq!(identity.version, parameters.identity.version);
        assert_eq!(
            identity.digest.to_string(),
            "sha256:89c0da5b058cf51b43953d0d31fe2e0f61f3c7038f9149e2fa59ad92c930ef71"
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
        let model_v3_catalog =
            RacingCatalogSnapshot::embedded_model_v3_thermal().expect("model V3 thermal catalog");

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
        assert!(
            model_v3_catalog
                .manifest()
                .compatibility
                .validate_for(
                    &crate::racing_model_v3_thermal_candidate_identity(),
                    ContractVersion::V1
                )
                .is_ok()
        );
        for legacy_model in [
            crate::racing_model_v1_identity(),
            crate::racing_model_v2_identity(),
        ] {
            assert!(
                model_v3_catalog
                    .manifest()
                    .compatibility
                    .validate_for(&legacy_model, ContractVersion::V1)
                    .is_err()
            );
        }
    }

    #[test]
    fn model_v3_catalog_resolves_the_exact_reviewed_thermal_profile() {
        let snapshot =
            RacingCatalogSnapshot::embedded_model_v3_thermal().expect("model V3 thermal catalog");
        let profile = snapshot
            .thermal_family_profile()
            .expect("thermal-family profile");
        let identity = snapshot
            .thermal_family_profile_identity()
            .expect("thermal-family profile identity");

        assert_eq!(identity, &profile.identity());
        assert_eq!(
            identity.digest.to_string(),
            crate::V3_THERMAL_FAMILY_PROFILE_DIGEST
        );
        assert!(snapshot.model_parameters().is_none());
        assert!(profile.resolve_vehicle("classic_v8_1960").is_ok());
        assert!(profile.resolve_vehicle("classic_v8_1970").is_ok());
        assert!(profile.resolve_vehicle("modern_v6t").is_ok());
        assert!(profile.resolve_vehicle("f1_2026").is_ok());
        assert!(profile.resolve_vehicle("unreviewed-prototype").is_err());
    }

    #[test]
    fn component_model_catalog_resolves_exact_profiles_and_capability_coverage() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed Model V3 catalog");
        assert_eq!(snapshot.manifest().catalog.version.to_string(), "1.6.0");
        assert_eq!(
            snapshot
                .power_unit_thermal_profile_identity()
                .expect("power-unit thermal identity"),
            &snapshot
                .power_unit_thermal_profile()
                .expect("power-unit thermal profile")
                .identity()
        );
        assert_eq!(
            snapshot
                .component_capability_profile()
                .expect("component capability profile")
                .components
                .len(),
            12
        );
        assert!(snapshot.component_capability_profile_identity().is_some());
        assert!(snapshot.thermal_family_profile().is_none());
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn component_capability_profile_fails_closed_on_incomplete_coverage() {
        let mut bundle = filesystem_bundle("1.6.0");
        let resource = bundle
            .resources
            .iter_mut()
            .find(|file| file.path == COMPONENT_CAPABILITY_PROFILE_PATH)
            .expect("component capability resource");
        let mut profile: ComponentCapabilityProfileV1 =
            serde_json::from_str(&resource.contents).expect("capability profile JSON");
        profile.components.pop();
        resource.contents = serde_json::to_string(&profile).expect("capability profile JSON");
        resign_bundle(&mut bundle);

        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(bundle),
            Err(RacingCatalogResolutionError::InvalidResolvedResources(_))
        ));
    }

    #[test]
    fn historical_catalogs_resolve_no_driver_instruction_profile() {
        for snapshot in [
            RacingCatalogSnapshot::embedded().expect("model V1 catalog"),
            RacingCatalogSnapshot::embedded_model_v2().expect("model V2 catalog"),
            RacingCatalogSnapshot::embedded_model_v3_thermal().expect("model V3 catalog"),
            RacingCatalogSnapshot::embedded_model_v3_component()
                .expect("component Model V3 catalog"),
        ] {
            assert!(snapshot.driver_instruction_profile().is_none());
            assert!(snapshot.driver_instruction_profile_identity().is_none());
        }
    }

    #[test]
    fn catalog_resolves_exact_driver_instruction_profile_bytes() {
        let bundle = bundle_with_driver_instruction_profile();
        let profile_bytes = bundle
            .resources
            .iter()
            .find(|resource| resource.path == DRIVER_INSTRUCTION_PROFILE_PATH)
            .expect("instruction profile bytes")
            .contents
            .as_bytes()
            .to_vec();
        let snapshot = RacingCatalogSnapshot::from_bundle(bundle).expect("profile catalog");
        let profile = snapshot
            .driver_instruction_profile()
            .expect("resolved instruction profile");
        let identity = snapshot
            .driver_instruction_profile_identity()
            .expect("instruction profile identity");

        assert_eq!(profile.default_mode, RacingDrivingMode::Balanced);
        assert_eq!(profile.max_events_per_session, 64);
        assert_eq!(identity.id.as_str(), RACING_DRIVER_INSTRUCTION_PROFILE_ID);
        assert_eq!(
            identity.version.to_string(),
            RACING_DRIVER_INSTRUCTION_PROFILE_VERSION
        );
        assert_eq!(identity.digest, Digest::from_bytes(&profile_bytes));
    }

    #[test]
    fn driver_instruction_profile_fails_closed_on_invalid_or_tampered_resources() {
        let mut invalid = bundle_with_driver_instruction_profile();
        let file = invalid
            .resources
            .iter_mut()
            .find(|resource| resource.path == DRIVER_INSTRUCTION_PROFILE_PATH)
            .expect("instruction profile");
        file.contents = file.contents.replace(
            "\"max_events_per_session\":64",
            "\"max_events_per_session\":0",
        );
        resign_bundle(&mut invalid);
        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(invalid),
            Err(RacingCatalogResolutionError::InvalidResolvedResources(reason))
                if reason.contains("event limit")
        ));

        let mut tampered = bundle_with_driver_instruction_profile();
        tampered
            .resources
            .iter_mut()
            .find(|resource| resource.path == DRIVER_INSTRUCTION_PROFILE_PATH)
            .expect("instruction profile")
            .contents
            .push(' ');
        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(tampered),
            Err(RacingCatalogResolutionError::ResourceDigestMismatch { path, .. })
                if path.as_str() == DRIVER_INSTRUCTION_PROFILE_PATH
        ));

        let mut mismatched_path = bundle_with_driver_instruction_profile();
        let mut index: RacingSimulationIndexV1 =
            serde_json::from_str(&mismatched_path.simulation_index).expect("simulation index");
        let resource = index
            .resources
            .iter_mut()
            .find(|resource| resource.id.as_str() == DRIVER_INSTRUCTION_PROFILE_RESOURCE_ID)
            .expect("instruction profile index entry");
        resource.path = "simulation/driver-instructions/other.json"
            .parse()
            .expect("other path");
        mismatched_path
            .resources
            .iter_mut()
            .find(|resource| resource.path == DRIVER_INSTRUCTION_PROFILE_PATH)
            .expect("instruction profile file")
            .path = "simulation/driver-instructions/other.json".to_string();
        mismatched_path.simulation_index =
            serde_json::to_string(&index).expect("simulation index JSON");
        resign_bundle(&mut mismatched_path);
        let error = RacingCatalogSnapshot::from_bundle(mismatched_path)
            .expect_err("mismatched profile path must fail");
        assert!(
            matches!(
                &error,
                RacingCatalogResolutionError::InvalidResolvedResources(reason)
                    if reason.contains("must use catalog resource ID")
            ),
            "unexpected error: {error:?}"
        );

        let mut duplicate = bundle_with_driver_instruction_profile();
        let mut index: RacingSimulationIndexV1 =
            serde_json::from_str(&duplicate.simulation_index).expect("simulation index");
        let profile_resource = index
            .resources
            .iter()
            .find(|resource| resource.id.as_str() == DRIVER_INSTRUCTION_PROFILE_RESOURCE_ID)
            .expect("instruction profile index entry")
            .clone();
        index.resources.push(profile_resource);
        index
            .resources
            .sort_by(|left, right| left.id.cmp(&right.id));
        duplicate.simulation_index = serde_json::to_string(&index).expect("simulation index JSON");
        resign_bundle(&mut duplicate);
        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(duplicate),
            Err(RacingCatalogResolutionError::InvalidIndex {
                pack: "simulation",
                ..
            })
        ));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn model_v3_catalog_rejects_a_resigned_but_modified_thermal_profile() {
        let mut bundle = filesystem_bundle("1.5.0");
        let profile = bundle
            .resources
            .iter_mut()
            .find(|file| file.path == THERMAL_FAMILY_PROFILE_PATH)
            .expect("thermal-family profile");
        profile.contents = profile.contents.replace(
            "\"thermal_capacity_multiplier\": 1.0",
            "\"thermal_capacity_multiplier\": 1.0001",
        );
        resign_bundle(&mut bundle);

        assert!(matches!(
            RacingCatalogSnapshot::from_bundle(bundle),
            Err(RacingCatalogResolutionError::InvalidResource { path, reason })
                if path.as_str() == THERMAL_FAMILY_PROFILE_PATH
                    && reason.contains("unsupported thermal family profile digest")
        ));
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
                if reason.contains("known Racing model")
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

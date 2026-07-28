//! Domain-neutral contracts for optional versioned resource catalogs.
//!
//! A resource catalog is a discovery and distribution boundary. Deterministic
//! execution remains bound to the selected simulation pack through
//! [`DeterministicRunContractV1::data_pack`].

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    ArtifactIdentity, CanonicalJsonError, ContractVersion, DeterministicRunContractV1, Digest,
    Identifier, SemanticVersion, canonical_json_digest,
};

const MAX_CATALOG_PATH_LENGTH: usize = 512;
const MAX_MEDIA_TYPE_LENGTH: usize = 127;

/// Wire version of a Resource Catalog manifest.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ResourceCatalogManifestVersion {
    /// First domain-neutral catalog release manifest.
    #[serde(rename = "pitgun.resource-catalog/v1")]
    V1,
}

/// Wire version of a Resource Catalog release identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum CatalogReleaseIdentityVersion {
    /// Identity derived from the canonical V1 manifest bytes.
    #[serde(rename = "pitgun.catalog-release-identity/v1")]
    V1,
}

/// Wire version of catalog compatibility declarations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum CatalogCompatibilityVersion {
    /// Exact contract and model versions supported by one release.
    #[serde(rename = "pitgun.catalog-compatibility/v1")]
    V1,
}

/// Media type of a catalog pack index.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum CatalogIndexMediaType {
    /// Strict UTF-8 JSON.
    #[serde(rename = "application/json")]
    ApplicationJson,
}

/// Safe immutable path relative to the root of a catalog release.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CatalogPath(String);

impl CatalogPath {
    /// Parses a portable lowercase POSIX-style relative path.
    pub fn new(value: impl Into<String>) -> Result<Self, CatalogContractError> {
        let value = value.into();
        validate_catalog_path(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical relative path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CatalogPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CatalogPath {
    type Err = CatalogContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for CatalogPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CatalogPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// Canonical lowercase Internet media type without parameters.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CatalogResourceMediaType(String);

impl CatalogResourceMediaType {
    /// Parses a portable media type such as `application/json` or `image/png`.
    pub fn new(value: impl Into<String>) -> Result<Self, CatalogContractError> {
        let value = value.into();
        validate_resource_media_type(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical media type.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CatalogResourceMediaType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CatalogResourceMediaType {
    type Err = CatalogContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for CatalogResourceMediaType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CatalogResourceMediaType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// Stable coordinates of an immutable catalog release.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRelease {
    /// Stable catalog identifier.
    pub id: Identifier,
    /// Exact release version.
    pub version: SemanticVersion,
}

/// Content identity of a complete canonical catalog manifest.
///
/// This value is external to the manifest so calculating the manifest digest
/// does not create a circular self-reference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogReleaseIdentityV1 {
    /// Exact identity wire version.
    pub schema_version: CatalogReleaseIdentityVersion,
    /// Stable catalog identifier.
    pub id: Identifier,
    /// Exact release version.
    pub version: SemanticVersion,
    /// SHA-256 of the complete canonical Resource Catalog manifest.
    pub manifest_digest: Digest,
}

/// Location and exact content identity of one pack index.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogIndexV1 {
    /// Immutable path relative to the catalog release root.
    pub path: CatalogPath,
    /// Exact index media type.
    pub media_type: CatalogIndexMediaType,
    /// SHA-256 of the canonical index bytes.
    pub digest: Digest,
}

/// Content-addressed resource entry reusable by domain-specific pack indexes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogResourceV1 {
    /// Stable resource identifier within its domain.
    pub id: Identifier,
    /// Immutable path relative to the catalog release root.
    pub path: CatalogPath,
    /// Exact media type of the stored bytes.
    pub media_type: CatalogResourceMediaType,
    /// SHA-256 of the exact stored bytes.
    pub digest: Digest,
}

/// One independently versioned and content-addressed catalog pack.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPackV1 {
    /// Stable pack identity used by consumers.
    pub identity: ArtifactIdentity,
    /// Canonical resource index whose digest defines the pack identity.
    pub index: CatalogIndexV1,
}

/// Exact model versions supported by one catalog release.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibleModelV1 {
    /// Stable model identifier.
    pub id: Identifier,
    /// Non-empty set of exact compatible model versions.
    pub versions: BTreeSet<SemanticVersion>,
}

/// Closed compatibility declaration for a catalog release.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCompatibilityV1 {
    /// Exact compatibility semantics.
    pub schema_version: CatalogCompatibilityVersion,
    /// Non-empty list of supported deterministic contract versions.
    pub contract_versions: Vec<ContractVersion>,
    /// Non-empty list ordered by model identifier.
    pub models: Vec<CompatibleModelV1>,
}

/// Complete immutable Resource Catalog release manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceCatalogManifestV1 {
    /// Exact manifest wire version.
    pub schema_version: ResourceCatalogManifestVersion,
    /// Stable release coordinates.
    pub catalog: CatalogRelease,
    /// Data that may influence deterministic execution.
    pub simulation_pack: CatalogPackV1,
    /// Application metadata that must not influence deterministic execution.
    pub presentation_pack: CatalogPackV1,
    /// Exact contracts and models supported by this release.
    pub compatibility: CatalogCompatibilityV1,
}

/// Structured catalog validation and compatibility failures.
#[derive(Debug)]
pub enum CatalogContractError {
    /// A catalog path is not one safe canonical relative path.
    InvalidPath(String),
    /// A resource media type is not one canonical lowercase type.
    InvalidMediaType(String),
    /// At least one deterministic contract version is required.
    EmptyContractVersions,
    /// Contract versions contain duplicates or are not in canonical order.
    NonCanonicalContractVersions,
    /// At least one compatible model is required.
    EmptyModels,
    /// Models contain duplicate identifiers or are not in canonical order.
    NonCanonicalModels,
    /// A compatible model has no declared exact version.
    EmptyModelVersions(Identifier),
    /// A pack identity does not match the digest of its canonical index.
    PackDigestMismatch {
        /// Manifest field containing the invalid pack.
        pack: &'static str,
        /// Digest declared by the pack identity.
        identity_digest: Digest,
        /// Digest declared for the canonical index.
        index_digest: Digest,
    },
    /// A supplied release identity does not identify this manifest.
    ReleaseIdentityMismatch {
        /// Identity calculated from the manifest.
        expected: Box<CatalogReleaseIdentityV1>,
        /// Identity supplied by the caller.
        actual: Box<CatalogReleaseIdentityV1>,
    },
    /// The run contract version is not accepted by this release.
    IncompatibleContractVersion(ContractVersion),
    /// The release does not declare the selected model identifier.
    UnknownModel(Identifier),
    /// The release does not support the selected exact model version.
    IncompatibleModelVersion {
        /// Selected stable model identifier.
        id: Identifier,
        /// Selected exact model version.
        version: SemanticVersion,
    },
    /// The run contract binds a different simulation pack.
    SimulationPackMismatch {
        /// Simulation pack required by the catalog.
        expected: Box<ArtifactIdentity>,
        /// Data pack bound by the run contract.
        actual: Box<ArtifactIdentity>,
    },
    /// Canonical serialization of the manifest failed.
    CanonicalJson(CanonicalJsonError),
}

impl fmt::Display for CatalogContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(value) => write!(
                formatter,
                "invalid catalog path {value:?}; expected a lowercase portable relative path"
            ),
            Self::InvalidMediaType(value) => write!(
                formatter,
                "invalid catalog media type {value:?}; expected lowercase type/subtype without parameters"
            ),
            Self::EmptyContractVersions => {
                formatter.write_str("catalog compatibility must declare a contract version")
            }
            Self::NonCanonicalContractVersions => formatter
                .write_str("catalog contract versions must be unique and in canonical order"),
            Self::EmptyModels => formatter.write_str("catalog compatibility must declare a model"),
            Self::NonCanonicalModels => {
                formatter.write_str("compatible models must be unique and ordered by identifier")
            }
            Self::EmptyModelVersions(id) => {
                write!(
                    formatter,
                    "compatible model {id} must declare an exact version"
                )
            }
            Self::PackDigestMismatch {
                pack,
                identity_digest,
                index_digest,
            } => write!(
                formatter,
                "{pack} identity digest {identity_digest} does not match index digest {index_digest}"
            ),
            Self::ReleaseIdentityMismatch { expected, actual } => write!(
                formatter,
                "catalog release identity mismatch: expected {}@{} {}, got {}@{} {}",
                expected.id,
                expected.version,
                expected.manifest_digest,
                actual.id,
                actual.version,
                actual.manifest_digest
            ),
            Self::IncompatibleContractVersion(version) => {
                write!(
                    formatter,
                    "catalog does not support contract version {version:?}"
                )
            }
            Self::UnknownModel(id) => write!(formatter, "catalog does not support model {id}"),
            Self::IncompatibleModelVersion { id, version } => {
                write!(
                    formatter,
                    "catalog does not support model {id} version {version}"
                )
            }
            Self::SimulationPackMismatch { expected, actual } => write!(
                formatter,
                "run contract data pack {}@{} {} does not match catalog simulation pack {}@{} {}",
                actual.id,
                actual.version,
                actual.digest,
                expected.id,
                expected.version,
                expected.digest
            ),
            Self::CanonicalJson(error) => {
                write!(formatter, "cannot calculate catalog identity: {error}")
            }
        }
    }
}

impl std::error::Error for CatalogContractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CanonicalJson(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CanonicalJsonError> for CatalogContractError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

impl CatalogCompatibilityV1 {
    /// Validates canonical ordering and non-empty compatibility sets.
    pub fn validate(&self) -> Result<(), CatalogContractError> {
        if self.contract_versions.is_empty() {
            return Err(CatalogContractError::EmptyContractVersions);
        }
        if self
            .contract_versions
            .windows(2)
            .any(|versions| versions[0] == versions[1])
        {
            return Err(CatalogContractError::NonCanonicalContractVersions);
        }
        if self.models.is_empty() {
            return Err(CatalogContractError::EmptyModels);
        }
        if self
            .models
            .windows(2)
            .any(|models| models[0].id >= models[1].id)
        {
            return Err(CatalogContractError::NonCanonicalModels);
        }
        if let Some(model) = self.models.iter().find(|model| model.versions.is_empty()) {
            return Err(CatalogContractError::EmptyModelVersions(model.id.clone()));
        }
        Ok(())
    }

    /// Validates one exact model and deterministic contract version.
    pub fn validate_for(
        &self,
        model: &ArtifactIdentity,
        contract_version: ContractVersion,
    ) -> Result<(), CatalogContractError> {
        self.validate()?;
        if !self.contract_versions.contains(&contract_version) {
            return Err(CatalogContractError::IncompatibleContractVersion(
                contract_version,
            ));
        }
        let compatible = self
            .models
            .iter()
            .find(|compatible| compatible.id == model.id)
            .ok_or_else(|| CatalogContractError::UnknownModel(model.id.clone()))?;
        if !compatible.versions.contains(&model.version) {
            return Err(CatalogContractError::IncompatibleModelVersion {
                id: model.id.clone(),
                version: model.version.clone(),
            });
        }
        Ok(())
    }
}

impl ResourceCatalogManifestV1 {
    /// Validates structural invariants independent of any selected run.
    pub fn validate(&self) -> Result<(), CatalogContractError> {
        validate_pack("simulation_pack", &self.simulation_pack)?;
        validate_pack("presentation_pack", &self.presentation_pack)?;
        self.compatibility.validate()
    }

    /// Derives the content identity of this complete canonical manifest.
    pub fn release_identity(&self) -> Result<CatalogReleaseIdentityV1, CatalogContractError> {
        self.validate()?;
        Ok(CatalogReleaseIdentityV1 {
            schema_version: CatalogReleaseIdentityVersion::V1,
            id: self.catalog.id.clone(),
            version: self.catalog.version.clone(),
            manifest_digest: canonical_json_digest(self)?,
        })
    }

    /// Verifies an externally supplied immutable release identity.
    pub fn verify_release_identity(
        &self,
        actual: &CatalogReleaseIdentityV1,
    ) -> Result<(), CatalogContractError> {
        let expected = self.release_identity()?;
        if expected == *actual {
            Ok(())
        } else {
            Err(CatalogContractError::ReleaseIdentityMismatch {
                expected: Box::new(expected),
                actual: Box::new(actual.clone()),
            })
        }
    }

    /// Validates compatibility and the V1 `data_pack` binding for one run.
    pub fn validate_for_run(
        &self,
        contract: &DeterministicRunContractV1,
    ) -> Result<(), CatalogContractError> {
        self.validate()?;
        self.compatibility
            .validate_for(&contract.model, contract.contract_version)?;
        if self.simulation_pack.identity != contract.data_pack {
            return Err(CatalogContractError::SimulationPackMismatch {
                expected: Box::new(self.simulation_pack.identity.clone()),
                actual: Box::new(contract.data_pack.clone()),
            });
        }
        Ok(())
    }
}

/// Validated in-memory boundary passed to a deterministic workload.
///
/// This is deliberately not serializable and does not define a new V1 wire
/// artifact. `Input` and `Resources` remain owned by the domain workload.
#[derive(Clone, Debug)]
pub struct ResolvedScenario<Input, Resources> {
    contract: DeterministicRunContractV1,
    catalog_release: Option<CatalogReleaseIdentityV1>,
    input: Input,
    resources: Resources,
}

impl<Input, Resources> ResolvedScenario<Input, Resources> {
    /// Creates a resolved scenario for a workload without a Resource Catalog.
    #[must_use]
    pub fn catalog_free(
        contract: DeterministicRunContractV1,
        input: Input,
        resources: Resources,
    ) -> Self {
        Self {
            contract,
            catalog_release: None,
            input,
            resources,
        }
    }

    /// Creates a resolved scenario after validating a catalog-backed run.
    pub fn catalog_backed(
        contract: DeterministicRunContractV1,
        catalog: &ResourceCatalogManifestV1,
        catalog_release: CatalogReleaseIdentityV1,
        input: Input,
        resources: Resources,
    ) -> Result<Self, CatalogContractError> {
        catalog.verify_release_identity(&catalog_release)?;
        catalog.validate_for_run(&contract)?;
        Ok(Self {
            contract,
            catalog_release: Some(catalog_release),
            input,
            resources,
        })
    }

    /// Returns the exact durable run contract.
    #[must_use]
    pub const fn contract(&self) -> &DeterministicRunContractV1 {
        &self.contract
    }

    /// Returns the optional catalog release used during resolution.
    #[must_use]
    pub const fn catalog_release(&self) -> Option<&CatalogReleaseIdentityV1> {
        self.catalog_release.as_ref()
    }

    /// Returns the validated domain input.
    #[must_use]
    pub const fn input(&self) -> &Input {
        &self.input
    }

    /// Returns the fully resolved domain resources.
    #[must_use]
    pub const fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Decomposes the boundary into its domain-owned values.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        DeterministicRunContractV1,
        Option<CatalogReleaseIdentityV1>,
        Input,
        Resources,
    ) {
        (
            self.contract,
            self.catalog_release,
            self.input,
            self.resources,
        )
    }
}

fn validate_pack(name: &'static str, pack: &CatalogPackV1) -> Result<(), CatalogContractError> {
    if pack.identity.digest == pack.index.digest {
        Ok(())
    } else {
        Err(CatalogContractError::PackDigestMismatch {
            pack: name,
            identity_digest: pack.identity.digest,
            index_digest: pack.index.digest,
        })
    }
}

fn validate_catalog_path(value: &str) -> Result<(), CatalogContractError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_CATALOG_PATH_LENGTH
        && !value.starts_with('/')
        && !value.ends_with('/')
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        });
    if valid {
        Ok(())
    } else {
        Err(CatalogContractError::InvalidPath(value.to_owned()))
    }
}

fn validate_resource_media_type(value: &str) -> Result<(), CatalogContractError> {
    let mut parts = value.split('/');
    let type_name = parts.next().unwrap_or_default();
    let subtype = parts.next().unwrap_or_default();
    let valid_token = |token: &str| {
        !token.is_empty()
            && token.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(
                        byte,
                        b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                    )
            })
    };
    if value.len() <= MAX_MEDIA_TYPE_LENGTH
        && parts.next().is_none()
        && valid_token(type_name)
        && valid_token(subtype)
    {
        Ok(())
    } else {
        Err(CatalogContractError::InvalidMediaType(value.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EventOrderingV1, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
        RandomAlgorithm, RandomContractV1, RuntimeProfile, ScenarioIdentity, Seed,
        StreamDerivation,
    };

    fn artifact(id: &str, version: &str, content: &[u8]) -> ArtifactIdentity {
        ArtifactIdentity {
            id: Identifier::new(id).expect("artifact id"),
            version: SemanticVersion::new(version).expect("artifact version"),
            digest: Digest::from_bytes(content),
        }
    }

    fn pack(id: &str, path: &str, content: &[u8]) -> CatalogPackV1 {
        let identity = artifact(id, "1.0.0", content);
        CatalogPackV1 {
            index: CatalogIndexV1 {
                path: CatalogPath::new(path).expect("catalog path"),
                media_type: CatalogIndexMediaType::ApplicationJson,
                digest: identity.digest,
            },
            identity,
        }
    }

    fn manifest() -> ResourceCatalogManifestV1 {
        ResourceCatalogManifestV1 {
            schema_version: ResourceCatalogManifestVersion::V1,
            catalog: CatalogRelease {
                id: Identifier::new("pitgun.racing").expect("catalog id"),
                version: SemanticVersion::new("1.0.0").expect("release version"),
            },
            simulation_pack: pack(
                "pitgun.racing.simulation",
                "simulation/index.json",
                b"simulation",
            ),
            presentation_pack: pack(
                "pitgun.racing.presentation",
                "presentation/index.json",
                b"presentation",
            ),
            compatibility: CatalogCompatibilityV1 {
                schema_version: CatalogCompatibilityVersion::V1,
                contract_versions: vec![ContractVersion::V1],
                models: vec![CompatibleModelV1 {
                    id: Identifier::new("pitgun.racing").expect("model id"),
                    versions: BTreeSet::from([
                        SemanticVersion::new("1.0.0").expect("model version")
                    ]),
                }],
            },
        }
    }

    fn contract(manifest: &ResourceCatalogManifestV1) -> DeterministicRunContractV1 {
        DeterministicRunContractV1 {
            contract_version: ContractVersion::V1,
            scenario: ScenarioIdentity {
                id: Identifier::new("racing.single-lap").expect("scenario id"),
                version: SemanticVersion::new("1.0.0").expect("scenario version"),
            },
            model: artifact("pitgun.racing", "1.0.0", b"model"),
            data_pack: manifest.simulation_pack.identity.clone(),
            runtime_profile: RuntimeProfile::PortableExactV1,
            random: RandomContractV1 {
                seed: Seed::new(42),
                algorithm: RandomAlgorithm::PitgunSplitMix64V1,
                stream_derivation: StreamDerivation::Sha256LabelV1,
            },
            clock: LogicalClockV1::new(0, 50_000, 1).expect("clock"),
            event_ordering: EventOrderingV1::v1(),
            input: InputIdentity {
                media_type: InputMediaType::ApplicationJson,
                canonicalization: InputCanonicalization::JcsRfc8785,
                digest: Digest::from_bytes(b"input"),
            },
        }
    }

    #[test]
    fn manifest_and_release_identity_round_trip_canonically() {
        let manifest = manifest();
        let identity = manifest.release_identity().expect("release identity");
        let json = serde_json::to_string(&manifest).expect("manifest JSON");
        let decoded: ResourceCatalogManifestV1 =
            serde_json::from_str(&json).expect("manifest round trip");

        assert_eq!(decoded, manifest);
        assert_eq!(
            decoded.release_identity().expect("decoded identity"),
            identity
        );
        decoded
            .verify_release_identity(&identity)
            .expect("matching identity");
    }

    #[test]
    fn presentation_change_does_not_change_run_identity() {
        let first = manifest();
        let run = contract(&first);
        let run_id = run.run_id().expect("run id");
        let mut second = first.clone();
        second.presentation_pack = pack(
            "pitgun.racing.presentation",
            "presentation/index.json",
            b"corrected labels",
        );

        first.validate_for_run(&run).expect("first release");
        second.validate_for_run(&run).expect("second release");
        assert_ne!(
            first.release_identity().expect("first identity"),
            second.release_identity().expect("second identity")
        );
        assert_eq!(run.run_id().expect("unchanged run id"), run_id);
    }

    #[test]
    fn simulation_change_requires_a_new_data_pack_binding() {
        let first = manifest();
        let run = contract(&first);
        let mut second = first.clone();
        second.simulation_pack = pack(
            "pitgun.racing.simulation",
            "simulation/index.json",
            b"changed physics data",
        );

        assert!(matches!(
            second.validate_for_run(&run),
            Err(CatalogContractError::SimulationPackMismatch { .. })
        ));
    }

    #[test]
    fn compatibility_failures_are_structured() {
        let manifest = manifest();
        let mut run = contract(&manifest);
        run.model.id = Identifier::new("pitgun.grid").expect("other model");
        assert!(matches!(
            manifest.validate_for_run(&run),
            Err(CatalogContractError::UnknownModel(_))
        ));

        run.model.id = Identifier::new("pitgun.racing").expect("racing model");
        run.model.version = SemanticVersion::new("2.0.0").expect("other version");
        assert!(matches!(
            manifest.validate_for_run(&run),
            Err(CatalogContractError::IncompatibleModelVersion { .. })
        ));
    }

    #[test]
    fn mutable_or_unsafe_paths_are_rejected() {
        for path in [
            "",
            "/simulation/index.json",
            "../index.json",
            "simulation/../index.json",
            "simulation//index.json",
            "Simulation/index.json",
            "https://catalog.pitgun.io/index.json",
        ] {
            assert!(CatalogPath::new(path).is_err(), "{path:?} must be rejected");
        }
    }

    #[test]
    fn resource_entries_are_typed_and_content_addressed() {
        let resource = CatalogResourceV1 {
            id: Identifier::new("circuit.monza").expect("resource id"),
            path: CatalogPath::new("simulation/circuits/monza.json").expect("resource path"),
            media_type: CatalogResourceMediaType::new("application/json").expect("media type"),
            digest: Digest::from_bytes(b"resource"),
        };
        let json = serde_json::to_string(&resource).expect("resource JSON");
        assert_eq!(
            serde_json::from_str::<CatalogResourceV1>(&json).expect("resource round trip"),
            resource
        );

        for media_type in [
            "",
            "application",
            "Application/JSON",
            "application/json; charset=utf-8",
            "application//json",
        ] {
            assert!(
                CatalogResourceMediaType::new(media_type).is_err(),
                "{media_type:?} must be rejected"
            );
        }
    }

    #[test]
    fn resolved_scenario_supports_catalog_backed_and_catalog_free_workloads() {
        let manifest = manifest();
        let run = contract(&manifest);
        let release = manifest.release_identity().expect("release identity");
        let catalog_backed =
            ResolvedScenario::catalog_backed(run.clone(), &manifest, release, "input", [1, 2])
                .expect("catalog-backed scenario");
        assert!(catalog_backed.catalog_release().is_some());
        assert_eq!(catalog_backed.resources(), &[1, 2]);

        let catalog_free = ResolvedScenario::catalog_free(run, "input", ());
        assert!(catalog_free.catalog_release().is_none());
    }

    #[test]
    fn unknown_schema_and_identity_versions_fail_closed() {
        let manifest_json = serde_json::to_string(&manifest()).expect("manifest JSON");
        let unknown_manifest =
            manifest_json.replace("pitgun.resource-catalog/v1", "pitgun.resource-catalog/v2");
        assert!(serde_json::from_str::<ResourceCatalogManifestV1>(&unknown_manifest).is_err());
        let unknown_compatibility = manifest_json.replace(
            "pitgun.catalog-compatibility/v1",
            "pitgun.catalog-compatibility/v2",
        );
        assert!(serde_json::from_str::<ResourceCatalogManifestV1>(&unknown_compatibility).is_err());
        let unknown_contract =
            manifest_json.replace("pitgun.deterministic-run/v1", "pitgun.deterministic-run/v2");
        assert!(serde_json::from_str::<ResourceCatalogManifestV1>(&unknown_contract).is_err());

        let identity = manifest().release_identity().expect("release identity");
        let identity_json = serde_json::to_string(&identity).expect("identity JSON");
        let unknown_identity = identity_json.replace(
            "pitgun.catalog-release-identity/v1",
            "pitgun.catalog-release-identity/v2",
        );
        assert!(serde_json::from_str::<CatalogReleaseIdentityV1>(&unknown_identity).is_err());
    }
}

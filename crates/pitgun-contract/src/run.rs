//! Typed identities and receipts for deterministic runs.
//!
//! These types implement the domain-neutral wire contract documented in
//! `docs/DETERMINISTIC_RUN_CONTRACT_V1.md`. They identify a logical computation
//! separately from any concrete native or WASM execution attempt.

use std::fmt;
use std::str::FromStr;

use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use uuid::{Uuid, Variant};

use crate::{CanonicalJsonError, Digest, canonical_json_digest};

const MAX_IDENTIFIER_LENGTH: usize = 128;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

/// Validation or receipt-verification errors for deterministic run contracts.
#[derive(Debug)]
pub enum RunContractError {
    /// An identifier does not use the stable lowercase ASCII grammar.
    InvalidIdentifier(String),
    /// A version is not one exact canonical SemVer value.
    InvalidSemanticVersion(String),
    /// A seed is not one canonical decimal `u64` string.
    InvalidSeed(String),
    /// A logical clock violates the fixed-step V1 invariants.
    InvalidClock(&'static str),
    /// Event keys do not define the exact V1 total order.
    InvalidEventOrdering,
    /// An execution identifier is not a canonical UUIDv7.
    InvalidExecutionId(String),
    /// A receipt belongs to a different logical run.
    RunIdMismatch {
        /// Run identity calculated from the supplied contract.
        expected: Digest,
        /// Run identity declared by the execution receipt.
        actual: Digest,
    },
    /// Canonical serialization of a validated contract failed.
    CanonicalJson(CanonicalJsonError),
}

impl fmt::Display for RunContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(value) => write!(
                formatter,
                "invalid identifier {value:?}; expected lowercase ASCII [a-z0-9][a-z0-9._-] with at most 128 characters"
            ),
            Self::InvalidSemanticVersion(value) => {
                write!(formatter, "invalid canonical semantic version: {value:?}")
            }
            Self::InvalidSeed(value) => {
                write!(formatter, "invalid canonical unsigned 64-bit seed: {value:?}")
            }
            Self::InvalidClock(reason) => write!(formatter, "invalid logical clock: {reason}"),
            Self::InvalidEventOrdering => formatter.write_str(
                "event ordering must be logical_tick, source_id, source_sequence, insertion_ordinal",
            ),
            Self::InvalidExecutionId(value) => {
                write!(formatter, "execution_id is not a canonical UUIDv7: {value:?}")
            }
            Self::RunIdMismatch { expected, actual } => {
                write!(formatter, "receipt run_id mismatch: expected {expected}, got {actual}")
            }
            Self::CanonicalJson(error) => write!(formatter, "cannot calculate run_id: {error}"),
        }
    }
}

impl std::error::Error for RunContractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CanonicalJson(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CanonicalJsonError> for RunContractError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

/// Stable lowercase identifier used for scenarios, models, packs, and runtimes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Identifier(String);

impl Identifier {
    /// Parses and validates a stable identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RunContractError> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for Identifier {
    type Err = RunContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Identifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// Exact canonical semantic version.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticVersion(Version);

impl SemanticVersion {
    /// Parses a canonical SemVer value without ranges or a leading `v`.
    pub fn new(value: impl AsRef<str>) -> Result<Self, RunContractError> {
        let value = value.as_ref();
        let parsed = Version::parse(value)
            .map_err(|_| RunContractError::InvalidSemanticVersion(value.to_string()))?;
        if parsed.to_string() != value {
            return Err(RunContractError::InvalidSemanticVersion(value.to_string()));
        }
        Ok(Self(parsed))
    }

    /// Returns the parsed SemVer value.
    #[must_use]
    pub const fn as_version(&self) -> &Version {
        &self.0
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SemanticVersion {
    type Err = RunContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for SemanticVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SemanticVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// Root deterministic seed encoded as a decimal string on JSON wires.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seed(u64);

impl Seed {
    /// Creates a seed from its lossless binary value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the binary seed value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for Seed {
    type Err = RunContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let canonical_digits = match value.as_bytes() {
            [b'0'] => true,
            [b'1'..=b'9', rest @ ..] => rest.iter().all(u8::is_ascii_digit),
            _ => false,
        };
        if !canonical_digits {
            return Err(RunContractError::InvalidSeed(value.to_string()));
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| RunContractError::InvalidSeed(value.to_string()))
    }
}

impl Serialize for Seed {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// Wire version of the deterministic run contract.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ContractVersion {
    /// `DeterministicRunContractV1` wire semantics.
    #[serde(rename = "pitgun.deterministic-run/v1")]
    V1,
}

/// Supported cross-runtime comparison profile.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RuntimeProfile {
    /// Exact canonical output and telemetry summary bytes on native and WASM.
    #[serde(rename = "portable-exact-v1")]
    PortableExactV1,
}

/// Stable random-number algorithm identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RandomAlgorithm {
    /// Explicit SplitMix64 algorithm defined by the Pitgun V1 test vectors.
    #[serde(rename = "pitgun-splitmix64-v1")]
    PitgunSplitMix64V1,
}

/// Stable independent-stream derivation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum StreamDerivation {
    /// SHA-256 derivation over canonical labeled input.
    #[serde(rename = "sha256-label-v1")]
    Sha256LabelV1,
}

/// Supported logical clock kind.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum LogicalClockKind {
    /// Rational fixed-step logical time independent of wall-clock time.
    #[serde(rename = "logical-fixed-step")]
    LogicalFixedStep,
}

/// Supported total-order event keys.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum EventOrderingKey {
    /// Integer logical tick.
    #[serde(rename = "logical_tick")]
    LogicalTick,
    /// Stable producer identifier.
    #[serde(rename = "source_id")]
    SourceId,
    /// Monotonic producer sequence.
    #[serde(rename = "source_sequence")]
    SourceSequence,
    /// Producer-assigned ordinal before concurrency or transport.
    #[serde(rename = "insertion_ordinal")]
    InsertionOrdinal,
}

/// String comparison rule for event ordering.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum StringOrder {
    /// Ascending Unicode scalar-value order.
    #[serde(rename = "unicode-code-point")]
    UnicodeCodePoint,
}

/// Canonical input media type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum InputMediaType {
    /// Strict UTF-8 JSON.
    #[serde(rename = "application/json")]
    ApplicationJson,
}

/// Canonical input serialization algorithm.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum InputCanonicalization {
    /// JSON Canonicalization Scheme from RFC 8785.
    #[serde(rename = "jcs-rfc8785")]
    JcsRfc8785,
}

/// Versioned identity of a scenario independent of any domain semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioIdentity {
    /// Stable scenario identifier.
    pub id: Identifier,
    /// Exact scenario semantics version.
    pub version: SemanticVersion,
}

/// Versioned and content-addressed model or data-pack identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    /// Stable artifact identifier.
    pub id: Identifier,
    /// Exact artifact semantics version.
    pub version: SemanticVersion,
    /// SHA-256 of the canonical artifact manifest.
    pub digest: Digest,
}

/// Deterministic random source configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomContractV1 {
    /// Losslessly encoded root seed.
    pub seed: Seed,
    /// Exact random-number algorithm.
    pub algorithm: RandomAlgorithm,
    /// Exact independent-stream derivation rule.
    pub stream_derivation: StreamDerivation,
}

/// Rational fixed-step logical clock.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LogicalClockV1 {
    kind: LogicalClockKind,
    epoch: i64,
    tick_numerator_us: u64,
    tick_denominator: u64,
}

impl LogicalClockV1 {
    /// Creates a reduced, positive fixed-step clock.
    pub fn new(
        epoch: i64,
        tick_numerator_us: u64,
        tick_denominator: u64,
    ) -> Result<Self, RunContractError> {
        if epoch.unsigned_abs() > MAX_SAFE_JSON_INTEGER {
            return Err(RunContractError::InvalidClock(
                "epoch is outside the exact I-JSON integer range",
            ));
        }
        if tick_numerator_us == 0 || tick_denominator == 0 {
            return Err(RunContractError::InvalidClock(
                "tick numerator and denominator must be positive",
            ));
        }
        if tick_numerator_us > MAX_SAFE_JSON_INTEGER || tick_denominator > MAX_SAFE_JSON_INTEGER {
            return Err(RunContractError::InvalidClock(
                "tick fraction is outside the exact I-JSON integer range",
            ));
        }
        if greatest_common_divisor(tick_numerator_us, tick_denominator) != 1 {
            return Err(RunContractError::InvalidClock(
                "tick fraction must be reduced to lowest terms",
            ));
        }
        Ok(Self {
            kind: LogicalClockKind::LogicalFixedStep,
            epoch,
            tick_numerator_us,
            tick_denominator,
        })
    }

    /// Returns the signed logical epoch in microseconds.
    #[must_use]
    pub const fn epoch(&self) -> i64 {
        self.epoch
    }

    /// Returns the tick-duration numerator in microseconds.
    #[must_use]
    pub const fn tick_numerator_us(&self) -> u64 {
        self.tick_numerator_us
    }

    /// Returns the tick-duration denominator.
    #[must_use]
    pub const fn tick_denominator(&self) -> u64 {
        self.tick_denominator
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogicalClockWire {
    kind: LogicalClockKind,
    epoch: i64,
    tick_numerator_us: u64,
    tick_denominator: u64,
}

impl<'de> Deserialize<'de> for LogicalClockV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = LogicalClockWire::deserialize(deserializer)?;
        if wire.kind != LogicalClockKind::LogicalFixedStep {
            return Err(de::Error::custom("unsupported logical clock kind"));
        }
        Self::new(wire.epoch, wire.tick_numerator_us, wire.tick_denominator)
            .map_err(de::Error::custom)
    }
}

/// Exact V1 total event order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EventOrderingV1 {
    keys: Vec<EventOrderingKey>,
    string_order: StringOrder,
}

impl EventOrderingV1 {
    /// Returns the required V1 total ordering.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            keys: required_event_keys().to_vec(),
            string_order: StringOrder::UnicodeCodePoint,
        }
    }

    /// Returns the ordered comparison keys.
    #[must_use]
    pub fn keys(&self) -> &[EventOrderingKey] {
        &self.keys
    }
}

impl Default for EventOrderingV1 {
    fn default() -> Self {
        Self::v1()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventOrderingWire {
    keys: Vec<EventOrderingKey>,
    string_order: StringOrder,
}

impl<'de> Deserialize<'de> for EventOrderingV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EventOrderingWire::deserialize(deserializer)?;
        if wire.keys.as_slice() != required_event_keys()
            || wire.string_order != StringOrder::UnicodeCodePoint
        {
            return Err(de::Error::custom(RunContractError::InvalidEventOrdering));
        }
        Ok(Self {
            keys: wire.keys,
            string_order: wire.string_order,
        })
    }
}

/// Identity of canonical input bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputIdentity {
    /// Supported input media type.
    pub media_type: InputMediaType,
    /// Canonical serialization used before hashing.
    pub canonicalization: InputCanonicalization,
    /// Digest of the canonical input bytes.
    pub digest: Digest,
}

/// Complete identity and execution semantics of one logical deterministic run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeterministicRunContractV1 {
    /// Exact wire-contract version.
    pub contract_version: ContractVersion,
    /// Scenario identity.
    pub scenario: ScenarioIdentity,
    /// Model identity shared by supported runtime targets.
    pub model: ArtifactIdentity,
    /// Data-pack identity.
    pub data_pack: ArtifactIdentity,
    /// Cross-runtime output comparison guarantee.
    pub runtime_profile: RuntimeProfile,
    /// Root randomness and stream semantics.
    pub random: RandomContractV1,
    /// Logical time semantics.
    pub clock: LogicalClockV1,
    /// Total event ordering semantics.
    pub event_ordering: EventOrderingV1,
    /// Canonical input identity.
    pub input: InputIdentity,
}

impl DeterministicRunContractV1 {
    /// Calculates the logical run identity from the complete canonical contract.
    pub fn run_id(&self) -> Result<Digest, CanonicalJsonError> {
        canonical_json_digest(self)
    }

    /// Verifies that a receipt declares this contract's logical run identity.
    pub fn verify_receipt(&self, receipt: &ExecutionReceiptV1) -> Result<(), RunContractError> {
        let expected = self.run_id()?;
        if receipt.run_id == expected {
            Ok(())
        } else {
            Err(RunContractError::RunIdMismatch {
                expected,
                actual: receipt.run_id,
            })
        }
    }
}

/// Opaque canonical UUIDv7 for one concrete execution attempt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionId(Uuid);

impl ExecutionId {
    /// Returns the parsed UUID value.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for ExecutionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(formatter)
    }
}

impl FromStr for ExecutionId {
    type Err = RunContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(value)
            .map_err(|_| RunContractError::InvalidExecutionId(value.to_string()))?;
        if uuid.get_version_num() != 7
            || uuid.get_variant() != Variant::RFC4122
            || uuid.hyphenated().to_string() != value
        {
            return Err(RunContractError::InvalidExecutionId(value.to_string()));
        }
        Ok(Self(uuid))
    }
}

impl Serialize for ExecutionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ExecutionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// Identity of the concrete engine artifact that executed a run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIdentity {
    /// Stable implementation identifier.
    pub engine: Identifier,
    /// Exact implementation version.
    pub engine_version: SemanticVersion,
    /// Exact registered compilation target.
    pub target: Identifier,
    /// Digest of the exact native binary or WASM module.
    pub artifact_digest: Digest,
}

/// Evidence produced by one concrete execution attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceiptV1 {
    /// Logical run identity shared across equivalent execution attempts.
    pub run_id: Digest,
    /// Unique UUIDv7 for this attempt.
    pub execution_id: ExecutionId,
    /// Concrete runtime evidence, excluded from `run_id`.
    pub runtime: RuntimeIdentity,
    /// Digest of the complete canonical domain output.
    pub output_digest: Digest,
    /// Digest of the canonical telemetry summary.
    pub telemetry_summary_digest: Digest,
}

impl ExecutionReceiptV1 {
    /// Creates a receipt bound to the supplied logical contract.
    pub fn for_contract(
        contract: &DeterministicRunContractV1,
        execution_id: ExecutionId,
        runtime: RuntimeIdentity,
        output_digest: Digest,
        telemetry_summary_digest: Digest,
    ) -> Result<Self, CanonicalJsonError> {
        Ok(Self {
            run_id: contract.run_id()?,
            execution_id,
            runtime,
            output_digest,
            telemetry_summary_digest,
        })
    }
}

fn validate_identifier(value: &str) -> Result<(), RunContractError> {
    let mut bytes = value.bytes();
    let first = bytes.next();
    let valid = value.len() <= MAX_IDENTIFIER_LENGTH
        && first.is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(RunContractError::InvalidIdentifier(value.to_string()))
    }
}

const fn required_event_keys() -> &'static [EventOrderingKey; 4] {
    &[
        EventOrderingKey::LogicalTick,
        EventOrderingKey::SourceId,
        EventOrderingKey::SourceSequence,
        EventOrderingKey::InsertionOrdinal,
    ]
}

const fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn digest(label: &str) -> Digest {
        Digest::from_bytes(label.as_bytes())
    }

    fn contract() -> DeterministicRunContractV1 {
        DeterministicRunContractV1 {
            contract_version: ContractVersion::V1,
            scenario: ScenarioIdentity {
                id: "example.constant-output".parse().expect("scenario id"),
                version: "1.0.0".parse().expect("scenario version"),
            },
            model: ArtifactIdentity {
                id: "pitgun.example".parse().expect("model id"),
                version: "1.2.3".parse().expect("model version"),
                digest: digest("model"),
            },
            data_pack: ArtifactIdentity {
                id: "pitgun.example.data".parse().expect("pack id"),
                version: "2.0.0".parse().expect("pack version"),
                digest: digest("data-pack"),
            },
            runtime_profile: RuntimeProfile::PortableExactV1,
            random: RandomContractV1 {
                seed: Seed::new(u64::MAX),
                algorithm: RandomAlgorithm::PitgunSplitMix64V1,
                stream_derivation: StreamDerivation::Sha256LabelV1,
            },
            clock: LogicalClockV1::new(0, 50_000, 1).expect("clock"),
            event_ordering: EventOrderingV1::v1(),
            input: InputIdentity {
                media_type: InputMediaType::ApplicationJson,
                canonicalization: InputCanonicalization::JcsRfc8785,
                digest: digest("input"),
            },
        }
    }

    fn runtime(target: &str) -> RuntimeIdentity {
        RuntimeIdentity {
            engine: "pitgun-rust".parse().expect("engine"),
            engine_version: "1.0.0".parse().expect("engine version"),
            target: target.parse().expect("target"),
            artifact_digest: digest(target),
        }
    }

    #[test]
    fn contract_round_trips_and_run_id_is_stable() {
        let contract = contract();
        let encoded = serde_json::to_string_pretty(&contract).expect("serialize contract");
        let decoded: DeterministicRunContractV1 =
            serde_json::from_str(&encoded).expect("deserialize contract");

        assert_eq!(decoded, contract);
        assert_eq!(
            contract.run_id().expect("run id").to_string(),
            "sha256:02caaa4c2cc82eaaa5a80e6bd30274e9abecdbeb5e3b5fb5e88777109934eda6"
        );
    }

    #[test]
    fn object_order_does_not_change_run_id() {
        let contract = contract();
        let mut value = serde_json::to_value(&contract).expect("contract value");
        let object = value.as_object_mut().expect("contract object");
        let scenario = object.remove("scenario").expect("scenario");
        object.insert("scenario".to_string(), scenario);
        let reordered: DeterministicRunContractV1 =
            serde_json::from_value(value).expect("reordered contract");

        assert_eq!(contract.run_id().unwrap(), reordered.run_id().unwrap());
    }

    #[test]
    fn every_semantic_identity_change_changes_run_id() {
        let original = contract();
        let original_id = original.run_id().expect("original run id");

        let mut changed_scenario = original.clone();
        changed_scenario.scenario.version = "1.0.1".parse().unwrap();
        let mut changed_model = original.clone();
        changed_model.model.digest = digest("new-model");
        let mut changed_pack = original.clone();
        changed_pack.data_pack.digest = digest("new-pack");
        let mut changed_seed = original.clone();
        changed_seed.random.seed = Seed::new(42);
        let mut changed_clock = original.clone();
        changed_clock.clock = LogicalClockV1::new(1, 50_000, 1).unwrap();
        let mut changed_input = original.clone();
        changed_input.input.digest = digest("new-input");

        for changed in [
            changed_scenario,
            changed_model,
            changed_pack,
            changed_seed,
            changed_clock,
            changed_input,
        ] {
            assert_ne!(original_id, changed.run_id().expect("changed run id"));
        }
    }

    #[test]
    fn strict_deserialization_rejects_invalid_contracts() {
        let valid = serde_json::to_value(contract()).expect("valid contract");

        let cases = [
            ("unknown field", {
                let mut value = valid.clone();
                value["typo"] = json!(true);
                value
            }),
            ("contract version", {
                let mut value = valid.clone();
                value["contract_version"] = json!("pitgun.deterministic-run/v2");
                value
            }),
            ("runtime profile", {
                let mut value = valid.clone();
                value["runtime_profile"] = json!("bounded-float-v1");
                value
            }),
            ("seed", {
                let mut value = valid.clone();
                value["random"]["seed"] = json!("007");
                value
            }),
            ("digest", {
                let mut value = valid.clone();
                value["input"]["digest"] = json!("sha256:ABC");
                value
            }),
            ("version", {
                let mut value = valid.clone();
                value["scenario"]["version"] = json!("v1.0.0");
                value
            }),
            ("event ordering", {
                let mut value = valid.clone();
                value["event_ordering"]["keys"]
                    .as_array_mut()
                    .unwrap()
                    .swap(0, 1);
                value
            }),
        ];

        for (name, value) in cases {
            assert!(
                serde_json::from_value::<DeterministicRunContractV1>(value).is_err(),
                "{name} must fail"
            );
        }
    }

    #[test]
    fn logical_clock_rejects_invalid_fractions() {
        assert!(LogicalClockV1::new(0, 0, 1).is_err());
        assert!(LogicalClockV1::new(0, 1, 0).is_err());
        assert!(LogicalClockV1::new(0, 100, 2).is_err());
        assert!(LogicalClockV1::new(i64::MAX, 1, 1).is_err());
    }

    #[test]
    fn seed_is_lossless_and_canonical() {
        let seed: Seed = "18446744073709551615".parse().expect("u64 max seed");

        assert_eq!(seed.get(), u64::MAX);
        assert_eq!(
            serde_json::to_string(&seed).unwrap(),
            "\"18446744073709551615\""
        );
        assert!("00".parse::<Seed>().is_err());
        assert!("+1".parse::<Seed>().is_err());
        assert!("18446744073709551616".parse::<Seed>().is_err());
    }

    #[test]
    fn receipt_runtime_does_not_change_logical_run_identity() {
        let contract = contract();
        let native = ExecutionReceiptV1::for_contract(
            &contract,
            "018f3b78-7e9a-7d20-a5e1-4ed92f02a591".parse().unwrap(),
            runtime("aarch64-apple-darwin"),
            digest("output"),
            digest("telemetry"),
        )
        .unwrap();
        let wasm = ExecutionReceiptV1::for_contract(
            &contract,
            "018f3b78-7e9a-7d20-a5e1-4ed92f02a592".parse().unwrap(),
            runtime("wasm32-unknown-unknown"),
            digest("output"),
            digest("telemetry"),
        )
        .unwrap();

        assert_eq!(native.run_id, wasm.run_id);
        assert_ne!(native.execution_id, wasm.execution_id);
        assert_ne!(native.runtime, wasm.runtime);
        contract.verify_receipt(&native).expect("native receipt");
        contract.verify_receipt(&wasm).expect("WASM receipt");
    }

    #[test]
    fn receipt_verification_rejects_a_different_contract() {
        let contract = contract();
        let receipt = ExecutionReceiptV1::for_contract(
            &contract,
            "018f3b78-7e9a-7d20-a5e1-4ed92f02a591".parse().unwrap(),
            runtime("wasm32-unknown-unknown"),
            digest("output"),
            digest("telemetry"),
        )
        .unwrap();
        let mut changed = contract;
        changed.random.seed = Seed::new(8);

        assert!(matches!(
            changed.verify_receipt(&receipt),
            Err(RunContractError::RunIdMismatch { .. })
        ));
    }

    #[test]
    fn receipt_round_trip_is_strict() {
        let contract = contract();
        let receipt = ExecutionReceiptV1::for_contract(
            &contract,
            "018f3b78-7e9a-7d20-a5e1-4ed92f02a591".parse().unwrap(),
            runtime("wasm32-unknown-unknown"),
            digest("output"),
            digest("telemetry"),
        )
        .unwrap();
        let encoded = serde_json::to_value(&receipt).expect("receipt value");
        let decoded: ExecutionReceiptV1 =
            serde_json::from_value(encoded.clone()).expect("receipt round trip");
        let mut unknown = encoded.clone();
        unknown["proof"] = json!("not-a-proof");
        let mut wrong_uuid = encoded;
        wrong_uuid["execution_id"] = json!("550e8400-e29b-41d4-a716-446655440000");

        assert_eq!(decoded, receipt);
        assert!(serde_json::from_value::<ExecutionReceiptV1>(unknown).is_err());
        assert!(serde_json::from_value::<ExecutionReceiptV1>(wrong_uuid).is_err());
    }

    #[test]
    fn execution_id_requires_canonical_uuid_v7() {
        assert!(
            "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
                .parse::<ExecutionId>()
                .is_ok()
        );
        assert!(
            "550e8400-e29b-41d4-a716-446655440000"
                .parse::<ExecutionId>()
                .is_err()
        );
        assert!(
            "018F3B78-7E9A-7D20-A5E1-4ED92F02A591"
                .parse::<ExecutionId>()
                .is_err()
        );
    }
}

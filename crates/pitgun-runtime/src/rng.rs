//! Stable deterministic random primitives for distributed compute.
//!
//! The algorithms in this module are compatibility contracts. Their names,
//! constants, byte order, and test vectors must not change in place.

use std::fmt;

use pitgun_contract::{CanonicalJsonError, Seed, canonical_json_bytes};
use serde::Serialize;
use sha2::{Digest as ShaDigest, Sha256};

const SPLITMIX_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const SPLITMIX_MIX_1: u64 = 0xBF58_476D_1CE4_E5B9;
const SPLITMIX_MIX_2: u64 = 0x94D0_49BB_1331_11EB;
const STREAM_DOMAIN: &[u8] = b"pitgun.sha256-label-v1\0";
const MAX_LABEL_BYTES: usize = 256;

/// Explicit SplitMix64 implementation identified as `pitgun-splitmix64-v1`.
///
/// This generator is deterministic and portable, but it is not cryptographic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitMix64V1 {
    state: u64,
}

impl SplitMix64V1 {
    /// Creates a generator whose internal state starts at the supplied seed.
    #[must_use]
    pub const fn from_seed(seed: Seed) -> Self {
        Self { state: seed.get() }
    }

    /// Creates a generator directly from a binary `u64` seed.
    #[must_use]
    pub const fn from_u64(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the current internal state before the next increment and mix.
    #[must_use]
    pub const fn state(&self) -> u64 {
        self.state
    }

    /// Advances the fixed-width state and returns the next 64 random bits.
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(SPLITMIX_GAMMA);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(SPLITMIX_MIX_1);
        value = (value ^ (value >> 27)).wrapping_mul(SPLITMIX_MIX_2);
        value ^ (value >> 31)
    }
}

/// Errors produced by `sha256-label-v1` stream derivation.
#[derive(Debug)]
pub enum StreamDerivationError {
    /// Component and entity labels must be non-empty.
    EmptyLabel {
        /// Name of the invalid label field.
        field: &'static str,
    },
    /// Labels are bounded to keep contracts and derivation inputs small.
    LabelTooLong {
        /// Name of the invalid label field.
        field: &'static str,
    },
    /// Control characters are forbidden in stream labels.
    ControlCharacter {
        /// Name of the invalid label field.
        field: &'static str,
    },
    /// Canonical JSON validation failed, including non-NFC Unicode.
    CanonicalJson(CanonicalJsonError),
}

impl fmt::Display for StreamDerivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLabel { field } => write!(formatter, "{field} must not be empty"),
            Self::LabelTooLong { field } => {
                write!(
                    formatter,
                    "{field} must not exceed {MAX_LABEL_BYTES} UTF-8 bytes"
                )
            }
            Self::ControlCharacter { field } => {
                write!(
                    formatter,
                    "{field} must not contain Unicode control characters"
                )
            }
            Self::CanonicalJson(error) => write!(formatter, "invalid stream label: {error}"),
        }
    }
}

impl std::error::Error for StreamDerivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CanonicalJson(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CanonicalJsonError> for StreamDerivationError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

/// Derives one independent stream seed with `sha256-label-v1`.
///
/// The hashed bytes are the ASCII domain `pitgun.sha256-label-v1`, one NUL
/// byte, then RFC 8785 canonical JSON for four strings in this exact order:
/// root seed, component id, entity id, and logical index. The returned seed is
/// the first eight SHA-256 bytes interpreted as an unsigned big-endian integer.
pub fn derive_stream_seed_v1(
    root_seed: Seed,
    component_id: &str,
    entity_id: &str,
    logical_index: u64,
) -> Result<Seed, StreamDerivationError> {
    validate_label("component_id", component_id)?;
    validate_label("entity_id", entity_id)?;

    #[derive(Serialize)]
    struct Labels<'a>(&'a str, &'a str, &'a str, &'a str);

    let root_seed = root_seed.to_string();
    let logical_index = logical_index.to_string();
    let labels = Labels(&root_seed, component_id, entity_id, &logical_index);
    let canonical_labels = canonical_json_bytes(&labels)?;

    let mut hasher = Sha256::new();
    hasher.update(STREAM_DOMAIN);
    hasher.update(canonical_labels);
    let digest = hasher.finalize();
    let mut seed_bytes = [0_u8; 8];
    seed_bytes.copy_from_slice(&digest[..8]);
    Ok(Seed::new(u64::from_be_bytes(seed_bytes)))
}

fn validate_label(field: &'static str, value: &str) -> Result<(), StreamDerivationError> {
    if value.is_empty() {
        return Err(StreamDerivationError::EmptyLabel { field });
    }
    if value.len() > MAX_LABEL_BYTES {
        return Err(StreamDerivationError::LabelTooLong { field });
    }
    if value.chars().any(char::is_control) {
        return Err(StreamDerivationError::ControlCharacter { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_seed_zero_matches_published_vectors() {
        let mut generator = SplitMix64V1::from_u64(0);
        let actual = std::array::from_fn::<_, 5, _>(|_| generator.next_u64());

        assert_eq!(
            actual,
            [
                0xE220_A839_7B1D_CDAF,
                0x6E78_9E6A_A1B9_65F4,
                0x06C4_5D18_8009_454F,
                0xF88B_B8A8_724C_81EC,
                0x1B39_896A_51A8_749B,
            ]
        );
        assert_eq!(generator.state(), 0x1715_609F_7C74_6C69);
    }

    #[test]
    fn splitmix64_max_seed_wraps_with_stable_vectors() {
        let mut generator = SplitMix64V1::from_u64(u64::MAX);
        let actual = [generator.next_u64(), generator.next_u64()];

        assert_eq!(actual, [0xE4D9_7177_1B65_2C20, 0xE99F_F867_DBF6_82C9]);
    }

    #[test]
    fn stream_derivation_matches_published_vectors() {
        let cases = [
            (0, "solver", "entity-0", 0, 0xD34D_C81F_E421_A5AD),
            (7, "racing.lap", "player", 1, 0x29E0_A030_58DC_9787),
            (
                u64::MAX,
                "grid.node",
                "poste-électrique",
                u64::MAX,
                0x34B6_B94D_D89B_109D,
            ),
        ];

        for (root, component, entity, index, expected) in cases {
            let actual = derive_stream_seed_v1(Seed::new(root), component, entity, index)
                .expect("stream seed");
            assert_eq!(actual.get(), expected);
        }
    }

    #[test]
    fn streams_are_independent_of_derivation_call_order() {
        let root = Seed::new(42);
        let first_a = derive_stream_seed_v1(root, "component-a", "entity", 0).unwrap();
        let first_b = derive_stream_seed_v1(root, "component-b", "entity", 0).unwrap();
        let second_b = derive_stream_seed_v1(root, "component-b", "entity", 0).unwrap();
        let second_a = derive_stream_seed_v1(root, "component-a", "entity", 0).unwrap();

        assert_eq!(first_a, second_a);
        assert_eq!(first_b, second_b);
        assert_ne!(first_a, first_b);
        assert_eq!(
            SplitMix64V1::from_seed(first_a).next_u64(),
            SplitMix64V1::from_seed(second_a).next_u64()
        );
    }

    #[test]
    fn every_derivation_dimension_is_domain_separated() {
        let root = Seed::new(7);
        let baseline = derive_stream_seed_v1(root, "component", "entity", 0).unwrap();
        let changed_root = derive_stream_seed_v1(Seed::new(8), "component", "entity", 0).unwrap();
        let changed_component = derive_stream_seed_v1(root, "other", "entity", 0).unwrap();
        let changed_entity = derive_stream_seed_v1(root, "component", "other", 0).unwrap();
        let changed_index = derive_stream_seed_v1(root, "component", "entity", 1).unwrap();

        for changed in [
            changed_root,
            changed_component,
            changed_entity,
            changed_index,
        ] {
            assert_ne!(baseline, changed);
        }
    }

    #[test]
    fn labels_require_nfc_and_reject_controls_or_empty_values() {
        assert!(derive_stream_seed_v1(Seed::new(1), "", "entity", 0).is_err());
        assert!(derive_stream_seed_v1(Seed::new(1), "component", "entity\n", 0).is_err());
        assert!(derive_stream_seed_v1(Seed::new(1), "component", "e\u{301}", 0).is_err());
        assert!(derive_stream_seed_v1(Seed::new(1), "component", "é", 0).is_ok());
    }
}

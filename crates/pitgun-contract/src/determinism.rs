//! Domain-neutral canonical JSON and content digest primitives.
//!
//! Deterministic identities must depend on semantic JSON content rather than
//! whitespace, object insertion order, or a runtime-specific serializer. This
//! module implements the strict input boundary used by
//! `DeterministicRunContractV1`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use sha2::{Digest as ShaDigest, Sha256};
use unicode_normalization::UnicodeNormalization;

const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

/// Errors produced while validating or canonicalizing JSON.
#[derive(Debug)]
pub enum CanonicalJsonError {
    /// The raw input is not one complete valid JSON value.
    InvalidJson(serde_json::Error),
    /// An object contains the same key more than once.
    DuplicateKey(String),
    /// A string or object key is not already NFC-normalized.
    NonNormalizedUnicode {
        /// JSON path of the invalid value.
        path: String,
    },
    /// An integer cannot be represented exactly by the I-JSON number model.
    UnsafeInteger {
        /// JSON path of the invalid number.
        path: String,
        /// Decimal representation of the rejected integer.
        value: String,
    },
    /// A floating-point value is not finite and therefore is not valid JSON.
    NonFiniteNumber {
        /// Structural path of the invalid value.
        path: String,
    },
    /// A serializable map uses a key that cannot be a JSON object key.
    InvalidMapKey {
        /// Structural path of the invalid map.
        path: String,
    },
    /// Serde could not expose the value for validation.
    ValueSerialization(serde_value::SerializerError),
    /// A serializable value cannot be represented as canonical JSON.
    Serialization(serde_json::Error),
}

impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "invalid JSON: {error}"),
            Self::DuplicateKey(key) => write!(formatter, "duplicate JSON object key: {key}"),
            Self::NonNormalizedUnicode { path } => {
                write!(formatter, "JSON string at {path} is not NFC-normalized")
            }
            Self::UnsafeInteger { path, value } => write!(
                formatter,
                "JSON integer at {path} is outside the exact I-JSON range: {value}"
            ),
            Self::NonFiniteNumber { path } => {
                write!(formatter, "non-finite number at {path} is not valid JSON")
            }
            Self::InvalidMapKey { path } => {
                write!(formatter, "map at {path} contains a non-string JSON key")
            }
            Self::ValueSerialization(error) => {
                write!(formatter, "value validation serialization failed: {error}")
            }
            Self::Serialization(error) => {
                write!(formatter, "canonical JSON serialization failed: {error}")
            }
        }
    }
}

impl std::error::Error for CanonicalJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidJson(error) | Self::Serialization(error) => Some(error),
            Self::ValueSerialization(error) => Some(error),
            Self::DuplicateKey(_)
            | Self::NonNormalizedUnicode { .. }
            | Self::UnsafeInteger { .. }
            | Self::NonFiniteNumber { .. }
            | Self::InvalidMapKey { .. } => None,
        }
    }
}

/// A strongly typed SHA-256 digest with canonical text encoding.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Calculates a SHA-256 digest over the exact byte slice.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Returns the raw 32-byte SHA-256 value.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(SHA256_PREFIX)?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Digest")
            .field(&self.to_string())
            .finish()
    }
}

impl Serialize for Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        encoded.parse().map_err(de::Error::custom)
    }
}

/// Errors produced while parsing a canonical digest string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DigestParseError {
    /// Only the exact lowercase `sha256:` algorithm prefix is accepted.
    InvalidAlgorithm,
    /// A SHA-256 digest must contain exactly 64 hexadecimal characters.
    InvalidLength,
    /// Digest bytes must use lowercase hexadecimal characters.
    InvalidHex { index: usize, character: char },
}

impl fmt::Display for DigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAlgorithm => {
                formatter.write_str("digest must start with lowercase sha256:")
            }
            Self::InvalidLength => {
                formatter.write_str("SHA-256 digest must contain 64 hex characters")
            }
            Self::InvalidHex { index, character } => write!(
                formatter,
                "invalid lowercase hexadecimal character {character:?} at digest index {index}"
            ),
        }
    }
}

impl std::error::Error for DigestParseError {}

impl FromStr for Digest {
    type Err = DigestParseError;

    fn from_str(encoded: &str) -> Result<Self, Self::Err> {
        let hexadecimal = encoded
            .strip_prefix(SHA256_PREFIX)
            .ok_or(DigestParseError::InvalidAlgorithm)?;
        if hexadecimal.len() != SHA256_HEX_LENGTH {
            return Err(DigestParseError::InvalidLength);
        }

        let mut bytes = [0_u8; 32];
        let hexadecimal = hexadecimal.as_bytes();
        for (index, pair) in hexadecimal.chunks_exact(2).enumerate() {
            let high = decode_lower_hex(pair[0], index * 2)?;
            let low = decode_lower_hex(pair[1], index * 2 + 1)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

/// Parses one strict JSON value and returns its RFC 8785 canonical bytes.
///
/// Unlike `serde_json::from_str`, this boundary rejects duplicate object keys.
pub fn canonicalize_json_str(input: &str) -> Result<Vec<u8>, CanonicalJsonError> {
    let value = parse_strict_json(input)?;
    canonicalize_value(&value)
}

/// Returns RFC 8785 canonical bytes for a serializable value.
///
/// The value is validated against Pitgun's I-JSON and NFC profile before the
/// canonical bytes are returned. Values containing non-finite floats fail in
/// the canonical serializer.
pub fn canonical_json_bytes<T>(value: &T) -> Result<Vec<u8>, CanonicalJsonError>
where
    T: Serialize,
{
    // serde_json maps non-finite floats to null, so validation must inspect the
    // typed Serde value before JSON serialization can lose that information.
    let validation_value =
        serde_value::to_value(value).map_err(CanonicalJsonError::ValueSerialization)?;
    validate_serializable_value(&validation_value, "$")?;
    serde_json_canonicalizer::to_vec(value).map_err(CanonicalJsonError::Serialization)
}

/// Calculates the digest of RFC 8785 canonical bytes for a serializable value.
pub fn canonical_json_digest<T>(value: &T) -> Result<Digest, CanonicalJsonError>
where
    T: Serialize,
{
    canonical_json_bytes(value).map(|bytes| Digest::from_bytes(&bytes))
}

fn canonicalize_value(value: &Value) -> Result<Vec<u8>, CanonicalJsonError> {
    validate_value(value, "$")?;
    serde_json_canonicalizer::to_vec(value).map_err(CanonicalJsonError::Serialization)
}

fn parse_strict_json(input: &str) -> Result<Value, CanonicalJsonError> {
    let duplicate = Rc::new(RefCell::new(None));
    let seed = StrictValueSeed {
        duplicate: Rc::clone(&duplicate),
    };
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let parsed = seed.deserialize(&mut deserializer);

    match parsed {
        Ok(value) => {
            deserializer
                .end()
                .map_err(CanonicalJsonError::InvalidJson)?;
            Ok(value)
        }
        Err(error) => match duplicate.borrow_mut().take() {
            Some(key) => Err(CanonicalJsonError::DuplicateKey(key)),
            None => Err(CanonicalJsonError::InvalidJson(error)),
        },
    }
}

fn validate_value(value: &Value, path: &str) -> Result<(), CanonicalJsonError> {
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                if value > MAX_SAFE_JSON_INTEGER {
                    return Err(CanonicalJsonError::UnsafeInteger {
                        path: path.to_string(),
                        value: value.to_string(),
                    });
                }
            } else if let Some(value) = number.as_i64()
                && value.unsigned_abs() > MAX_SAFE_JSON_INTEGER
            {
                return Err(CanonicalJsonError::UnsafeInteger {
                    path: path.to_string(),
                    value: value.to_string(),
                });
            }
            Ok(())
        }
        Value::String(string) => validate_nfc(string, path),
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_value(value, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (key, value) in values {
                validate_nfc(key, &format!("{path}.<key>"))?;
                validate_value(value, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
    }
}

fn validate_serializable_value(
    value: &serde_value::Value,
    path: &str,
) -> Result<(), CanonicalJsonError> {
    use serde_value::Value as SerdeValue;

    match value {
        SerdeValue::Bool(_) | SerdeValue::Unit => Ok(()),
        SerdeValue::U8(value) => validate_unsigned_integer(u64::from(*value), path),
        SerdeValue::U16(value) => validate_unsigned_integer(u64::from(*value), path),
        SerdeValue::U32(value) => validate_unsigned_integer(u64::from(*value), path),
        SerdeValue::U64(value) => validate_unsigned_integer(*value, path),
        SerdeValue::I8(value) => validate_signed_integer(i64::from(*value), path),
        SerdeValue::I16(value) => validate_signed_integer(i64::from(*value), path),
        SerdeValue::I32(value) => validate_signed_integer(i64::from(*value), path),
        SerdeValue::I64(value) => validate_signed_integer(*value, path),
        SerdeValue::F32(value) if !value.is_finite() => Err(CanonicalJsonError::NonFiniteNumber {
            path: path.to_string(),
        }),
        SerdeValue::F64(value) if !value.is_finite() => Err(CanonicalJsonError::NonFiniteNumber {
            path: path.to_string(),
        }),
        SerdeValue::F32(_) | SerdeValue::F64(_) => Ok(()),
        SerdeValue::Char(value) => validate_nfc(&value.to_string(), path),
        SerdeValue::String(value) => validate_nfc(value, path),
        SerdeValue::Option(None) => Ok(()),
        SerdeValue::Option(Some(value)) | SerdeValue::Newtype(value) => {
            validate_serializable_value(value, path)
        }
        SerdeValue::Seq(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_serializable_value(value, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        SerdeValue::Map(values) => {
            for (key, value) in values {
                let key = match key {
                    SerdeValue::String(key) => key.clone(),
                    SerdeValue::Char(key) => key.to_string(),
                    _ => {
                        return Err(CanonicalJsonError::InvalidMapKey {
                            path: path.to_string(),
                        });
                    }
                };
                validate_nfc(&key, &format!("{path}.<key>"))?;
                validate_serializable_value(value, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        SerdeValue::Bytes(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_unsigned_integer(u64::from(*value), &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
    }
}

fn validate_unsigned_integer(value: u64, path: &str) -> Result<(), CanonicalJsonError> {
    if value > MAX_SAFE_JSON_INTEGER {
        Err(CanonicalJsonError::UnsafeInteger {
            path: path.to_string(),
            value: value.to_string(),
        })
    } else {
        Ok(())
    }
}

fn validate_signed_integer(value: i64, path: &str) -> Result<(), CanonicalJsonError> {
    if value.unsigned_abs() > MAX_SAFE_JSON_INTEGER {
        Err(CanonicalJsonError::UnsafeInteger {
            path: path.to_string(),
            value: value.to_string(),
        })
    } else {
        Ok(())
    }
}

fn validate_nfc(value: &str, path: &str) -> Result<(), CanonicalJsonError> {
    if value.nfc().eq(value.chars()) {
        Ok(())
    } else {
        Err(CanonicalJsonError::NonNormalizedUnicode {
            path: path.to_string(),
        })
    }
}

fn decode_lower_hex(byte: u8, index: usize) -> Result<u8, DigestParseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(DigestParseError::InvalidHex {
            index,
            character: char::from(byte),
        }),
    }
}

struct StrictValueSeed {
    duplicate: Rc<RefCell<Option<String>>>,
}

impl<'de> DeserializeSeed<'de> for StrictValueSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictValueVisitor {
            duplicate: self.duplicate,
        })
    }
}

struct StrictValueVisitor {
    duplicate: Rc<RefCell<Option<String>>>,
}

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a valid JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrictValueSeed {
            duplicate: self.duplicate,
        }
        .deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(StrictValueSeed {
            duplicate: Rc::clone(&self.duplicate),
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                self.duplicate.replace(Some(key.clone()));
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            let value = object.next_value_seed(StrictValueSeed {
                duplicate: Rc::clone(&self.duplicate),
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::json;

    use super::*;

    #[test]
    fn canonicalizes_reordered_objects_to_published_bytes() {
        let first = canonicalize_json_str(r#"{"c": 1.2e2, "b": false, "a": "Hello!"}"#)
            .expect("first input");
        let reordered =
            canonicalize_json_str(r#"{"a":"Hello!","c":120,"b":false}"#).expect("reordered input");

        assert_eq!(first, br#"{"a":"Hello!","b":false,"c":120}"#);
        assert_eq!(first, reordered);
    }

    #[test]
    fn canonicalizes_rfc_8785_number_forms() {
        let canonical = canonicalize_json_str(
            "[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001]",
        )
        .expect("RFC 8785 numbers");

        assert_eq!(canonical, b"[333333333.3333333,1e+30,4.5,0.002,1e-27]");
    }

    #[test]
    fn preserves_array_order_and_canonical_unicode_bytes() {
        let canonical =
            canonicalize_json_str(r#"{"é":["café",2,1]}"#).expect("canonical Unicode array");

        assert_eq!(canonical, "{\"é\":[\"café\",2,1]}".as_bytes());
    }

    #[test]
    fn canonical_digest_has_a_stable_published_vector() {
        let value = json!({"a": "Hello!", "b": false, "c": 120});
        let digest = canonical_json_digest(&value).expect("canonical digest");

        assert_eq!(
            digest.to_string(),
            "sha256:eade68c3df1936c19da4955a9e876474d4b3e52e016e6b02ed354d9c42a49513"
        );
    }

    #[test]
    fn semantic_change_changes_the_digest() {
        let first = canonical_json_digest(&json!({"value": 1})).expect("first digest");
        let second = canonical_json_digest(&json!({"value": 2})).expect("second digest");

        assert_ne!(first, second);
    }

    #[test]
    fn rejects_duplicate_keys_at_any_depth() {
        let error = canonicalize_json_str(r#"{"outer":{"same":1,"same":2}}"#)
            .expect_err("duplicate key must fail");

        assert!(matches!(error, CanonicalJsonError::DuplicateKey(key) if key == "same"));
    }

    #[test]
    fn rejects_non_normalized_unicode() {
        let error = canonicalize_json_str("{\"value\":\"e\\u0301\"}")
            .expect_err("decomposed unicode must fail");

        assert!(matches!(
            error,
            CanonicalJsonError::NonNormalizedUnicode { path } if path == "$.value"
        ));
        canonicalize_json_str("{\"value\":\"é\"}").expect("NFC unicode must pass");
    }

    #[test]
    fn rejects_unsafe_json_integers() {
        let error = canonicalize_json_str(r#"{"value":9007199254740992}"#)
            .expect_err("unsafe integer must fail");

        assert!(matches!(
            error,
            CanonicalJsonError::UnsafeInteger { path, value }
                if path == "$.value" && value == "9007199254740992"
        ));
    }

    #[test]
    fn rejects_unsafe_integers_in_serializable_values() {
        let error = canonical_json_bytes(&json!({"value": MAX_SAFE_JSON_INTEGER + 1}))
            .expect_err("unsafe typed integer must fail");

        assert!(matches!(
            error,
            CanonicalJsonError::UnsafeInteger { path, value }
                if path == "$.value" && value == "9007199254740992"
        ));
    }

    #[derive(Serialize)]
    struct NonFiniteValue {
        value: f64,
    }

    #[test]
    fn rejects_non_finite_serializable_values() {
        let error =
            canonical_json_bytes(&NonFiniteValue { value: f64::NAN }).expect_err("NaN must fail");

        assert!(matches!(
            error,
            CanonicalJsonError::NonFiniteNumber { path } if path == "$.value"
        ));
    }

    #[test]
    fn digest_round_trips_through_text_and_json() {
        let digest = Digest::from_bytes(b"pitgun");
        let encoded = digest.to_string();
        let parsed: Digest = encoded.parse().expect("digest text");
        let json = serde_json::to_string(&digest).expect("digest JSON");
        let decoded: Digest = serde_json::from_str(&json).expect("digest from JSON");

        assert_eq!(parsed, digest);
        assert_eq!(decoded, digest);
        assert_eq!(digest.as_bytes().len(), 32);
    }

    #[test]
    fn digest_parser_rejects_non_canonical_encodings() {
        let valid = Digest::from_bytes(b"pitgun").to_string();
        let uppercase = valid.to_uppercase();
        let uppercase_hex = format!("sha256:A{}", &valid[8..]);
        let wrong_algorithm = valid.replacen("sha256:", "md5:", 1);
        let short = &valid[..valid.len() - 1];

        assert!(matches!(
            uppercase.parse::<Digest>(),
            Err(DigestParseError::InvalidAlgorithm)
        ));
        assert!(matches!(
            uppercase_hex.parse::<Digest>(),
            Err(DigestParseError::InvalidHex {
                index: 0,
                character: 'A'
            })
        ));
        assert!(matches!(
            wrong_algorithm.parse::<Digest>(),
            Err(DigestParseError::InvalidAlgorithm)
        ));
        assert!(matches!(
            short.parse::<Digest>(),
            Err(DigestParseError::InvalidLength)
        ));
    }
}

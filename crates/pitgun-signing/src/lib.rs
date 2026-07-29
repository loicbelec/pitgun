use std::collections::BTreeMap;
use std::fmt;

use hmac::{Hmac, Mac};
use pitgun_contract::{
    AuthorizationSignatureAlgorithm, Identifier, RunAuthorizationError, SignedRunAuthorizationV1,
};
use sha2::Sha256;

pub const SIGNING_SECRET_ENV: &str = "PITGUN_SIGNING_SECRET";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug)]
pub enum SigningError {
    MissingSecret,
    EmptySecret,
}

impl std::fmt::Display for SigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SigningError::MissingSecret => {
                write!(f, "{SIGNING_SECRET_ENV} is not set")
            }
            SigningError::EmptySecret => write!(f, "{SIGNING_SECRET_ENV} must not be empty"),
        }
    }
}

impl std::error::Error for SigningError {}

#[derive(Clone)]
pub struct SigningKey {
    secret: Vec<u8>,
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SigningKey([REDACTED])")
    }
}

impl SigningKey {
    pub fn from_env() -> Result<Self, SigningError> {
        let raw = std::env::var(SIGNING_SECRET_ENV).map_err(|_| SigningError::MissingSecret)?;
        Self::from_secret(raw.trim().as_bytes())
    }

    pub fn from_secret(secret: &[u8]) -> Result<Self, SigningError> {
        if secret.is_empty() {
            return Err(SigningError::EmptySecret);
        }

        Ok(Self {
            secret: secret.to_vec(),
        })
    }

    pub fn sign(&self, bytes: &[u8]) -> String {
        let mut mac =
            HmacSha256::new_from_slice(&self.secret).expect("HMAC can take a key of any size");
        mac.update(bytes);
        let signature = mac.finalize().into_bytes();
        hex::encode(signature)
    }

    pub fn verify(&self, bytes: &[u8], signature: &str) -> bool {
        let Ok(expected) = hex::decode(signature) else {
            return false;
        };

        let mut mac =
            HmacSha256::new_from_slice(&self.secret).expect("HMAC can take a key of any size");
        mac.update(bytes);
        mac.verify_slice(&expected).is_ok()
    }
}

/// Verification material retained by stable key identifier.
///
/// Authorities sign with one active key. Verifiers retain previous keys until
/// every authorization issued by them has passed expiry and late-submission
/// grace.
#[derive(Clone, Debug, Default)]
pub struct VerificationKeyring {
    keys: BTreeMap<Identifier, SigningKey>,
}

impl VerificationKeyring {
    /// Creates an empty keyring.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keys: BTreeMap::new(),
        }
    }

    /// Adds or replaces verification material for an exact key identifier.
    pub fn insert(&mut self, key_id: Identifier, key: SigningKey) {
        self.keys.insert(key_id, key);
    }

    /// Verifies an authorization before starting a new execution.
    pub fn verify_execution(
        &self,
        signed: &SignedRunAuthorizationV1,
        expected_audience: &Identifier,
        now_ms: i64,
    ) -> Result<(), AuthorizationVerificationError> {
        self.verify_signature_and_audience(signed, expected_audience)?;
        signed
            .authorization
            .validity
            .validate_execution_time(now_ms)
            .map_err(AuthorizationVerificationError::Authorization)
    }

    /// Verifies an authorization before accepting a completed result.
    ///
    /// Successful cryptographic verification does not consume the nonce. The
    /// persistence boundary must atomically reject a nonce already used for a
    /// prior accepted submission.
    pub fn verify_submission(
        &self,
        signed: &SignedRunAuthorizationV1,
        expected_audience: &Identifier,
        now_ms: i64,
    ) -> Result<(), AuthorizationVerificationError> {
        self.verify_signature_and_audience(signed, expected_audience)?;
        signed
            .authorization
            .validity
            .validate_submission_time(now_ms)
            .map_err(AuthorizationVerificationError::Authorization)
    }

    fn verify_signature_and_audience(
        &self,
        signed: &SignedRunAuthorizationV1,
        expected_audience: &Identifier,
    ) -> Result<(), AuthorizationVerificationError> {
        if signed.algorithm != AuthorizationSignatureAlgorithm::HmacSha256 {
            return Err(AuthorizationVerificationError::UnsupportedAlgorithm);
        }
        if &signed.authorization.audience != expected_audience {
            return Err(AuthorizationVerificationError::AudienceMismatch);
        }
        let key = self
            .keys
            .get(&signed.authorization.signing_key_id)
            .ok_or(AuthorizationVerificationError::UnknownKey)?;
        let bytes = signed
            .authorization
            .signing_bytes()
            .map_err(AuthorizationVerificationError::Authorization)?;
        if !key.verify(&bytes, &signed.signature) {
            return Err(AuthorizationVerificationError::InvalidSignature);
        }
        Ok(())
    }
}

/// Fail-closed errors returned while verifying authority output.
#[derive(Debug)]
pub enum AuthorizationVerificationError {
    /// The declared algorithm is not accepted by this verifier.
    UnsupportedAlgorithm,
    /// The authorization targets another service.
    AudienceMismatch,
    /// Verification material for the declared key ID is unavailable.
    UnknownKey,
    /// The detached signature does not protect the supplied canonical bytes.
    InvalidSignature,
    /// The authorization payload or validity window is invalid.
    Authorization(RunAuthorizationError),
}

impl fmt::Display for AuthorizationVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedAlgorithm => {
                formatter.write_str("unsupported authorization signature algorithm")
            }
            Self::AudienceMismatch => formatter.write_str("authorization audience mismatch"),
            Self::UnknownKey => formatter.write_str("unknown authorization signing key"),
            Self::InvalidSignature => formatter.write_str("invalid authorization signature"),
            Self::Authorization(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AuthorizationVerificationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authorization(error) => Some(error),
            _ => None,
        }
    }
}

pub fn sign(bytes: &[u8]) -> Result<String, SigningError> {
    SigningKey::from_env().map(|key| key.sign(bytes))
}

pub fn verify(bytes: &[u8], signature: &str) -> Result<bool, SigningError> {
    SigningKey::from_env().map(|key| key.verify(bytes, signature))
}

#[cfg(test)]
mod tests {
    use pitgun_contract::SignedRunAuthorizationV1;

    use super::*;

    const SIGNED_FIXTURE: &str = r#"{
      "authorization": {
        "authorization_version": "pitgun.run-authorization/v1",
        "nonce": "sha256:78377b525757b4944d54b623455b1bde763075c4c35f79e1c44180c10afc1345",
        "subject": "career.123",
        "audience": "pitgun.verifier",
        "contract": {
          "contract_version": "pitgun.deterministic-run/v1",
          "scenario": {"id": "racing.race", "version": "1.0.0"},
          "model": {
            "id": "pitgun.racing",
            "version": "1.0.0",
            "digest": "sha256:9372c470eeadd5ec6e03f2f23e39e0d70980f8d242b158c71dd343ee57bbfa26"
          },
          "data_pack": {
            "id": "pitgun.racing.simulation",
            "version": "1.0.0",
            "digest": "sha256:7e12cf820b80991844b2934f9d8f4a2f812f93239150e3c67a5a0b728e1121e8"
          },
          "runtime_profile": "portable-exact-v1",
          "random": {
            "seed": "42",
            "algorithm": "pitgun-splitmix64-v1",
            "stream_derivation": "sha256-label-v1"
          },
          "clock": {
            "kind": "logical-fixed-step",
            "epoch": 0,
            "tick_numerator_us": 50000,
            "tick_denominator": 1
          },
          "event_ordering": {
            "keys": ["logical_tick", "source_id", "source_sequence", "insertion_ordinal"],
            "string_order": "unicode-code-point"
          },
          "input": {
            "media_type": "application/json",
            "canonicalization": "jcs-rfc8785",
            "digest": "sha256:c96c6d5be8d08a12e7b5cdc1153c6717527fd75a497577d9345d39189250a969"
          }
        },
        "run_id": "sha256:ebfd6f773befde72b90d564c04bb15904e79229378f9e4dea27457a965be3355",
        "policy": {
          "id": "pitgun.racing.tuning",
          "version": "1.0.0",
          "digest": "sha256:823412d1eacb0f7a7b4ab4e4ab7af9757d51bc7c31f50da4f5d1a8f439c5c0aa"
        },
        "signing_key_id": "staging-2026-07",
        "validity": {
          "issued_at_ms": 1710000000000,
          "expires_at_ms": 1710000300000,
          "late_submission_grace_ms": 120000
        }
      },
      "algorithm": "hmac-sha256",
      "signature": ""
    }"#;

    fn signed_fixture() -> SignedRunAuthorizationV1 {
        let mut signed: SignedRunAuthorizationV1 =
            serde_json::from_str(SIGNED_FIXTURE).expect("signed fixture");
        signed.authorization.run_id = signed
            .authorization
            .contract
            .run_id()
            .expect("fixture run id");
        let key = SigningKey::from_secret(b"rotation-test-secret").expect("key");
        signed.signature = key.sign(&signed.authorization.signing_bytes().expect("signing bytes"));
        signed
    }

    #[test]
    fn signing_key_debug_never_exposes_secret() {
        let key = SigningKey::from_secret(b"very-private-secret").expect("key");
        let debug = format!("{key:?}");
        assert_eq!(debug, "SigningKey([REDACTED])");
        assert!(!debug.contains("very-private-secret"));
    }

    #[test]
    fn retained_key_verifies_execution_and_late_submission() {
        let signed = signed_fixture();
        let mut keyring = VerificationKeyring::new();
        keyring.insert(
            "staging-2026-07".parse().expect("key id"),
            SigningKey::from_secret(b"rotation-test-secret").expect("key"),
        );
        let audience = "pitgun.verifier".parse().expect("audience");

        keyring
            .verify_execution(&signed, &audience, 1_710_000_300_000)
            .expect("execution at expiry");
        keyring
            .verify_submission(&signed, &audience, 1_710_000_420_000)
            .expect("submission at grace deadline");
    }

    #[test]
    fn wrong_key_audience_and_expired_window_fail_closed() {
        let signed = signed_fixture();
        let audience = "pitgun.verifier".parse().expect("audience");
        let mut keyring = VerificationKeyring::new();
        assert!(matches!(
            keyring.verify_execution(&signed, &audience, 1_710_000_100_000),
            Err(AuthorizationVerificationError::UnknownKey)
        ));

        keyring.insert(
            "staging-2026-07".parse().expect("key id"),
            SigningKey::from_secret(b"wrong-secret").expect("key"),
        );
        assert!(matches!(
            keyring.verify_execution(&signed, &audience, 1_710_000_100_000),
            Err(AuthorizationVerificationError::InvalidSignature)
        ));

        keyring.insert(
            "staging-2026-07".parse().expect("key id"),
            SigningKey::from_secret(b"rotation-test-secret").expect("key"),
        );
        assert!(matches!(
            keyring.verify_execution(
                &signed,
                &"another.verifier".parse().expect("other audience"),
                1_710_000_100_000
            ),
            Err(AuthorizationVerificationError::AudienceMismatch)
        ));
        assert!(matches!(
            keyring.verify_execution(&signed, &audience, 1_710_000_300_001),
            Err(AuthorizationVerificationError::Authorization(
                RunAuthorizationError::Expired
            ))
        ));
    }
}

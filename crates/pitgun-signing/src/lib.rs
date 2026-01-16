use hmac::{Hmac, Mac};
use pitgun_contract::game::v1::{
    GameSimulationContractPayloadV1, GameSimulationContractV1, SigningBytesError,
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

#[derive(Debug)]
pub enum SigningRequestError {
    Signing(SigningError),
    SigningBytes(SigningBytesError),
}

impl std::fmt::Display for SigningRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SigningRequestError::Signing(err) => write!(f, "{err}"),
            SigningRequestError::SigningBytes(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for SigningRequestError {}

impl From<SigningError> for SigningRequestError {
    fn from(value: SigningError) -> Self {
        SigningRequestError::Signing(value)
    }
}

impl From<SigningBytesError> for SigningRequestError {
    fn from(value: SigningBytesError) -> Self {
        SigningRequestError::SigningBytes(value)
    }
}

#[derive(Clone, Debug)]
pub struct SigningKey {
    secret: Vec<u8>,
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

pub fn sign(bytes: &[u8]) -> Result<String, SigningError> {
    SigningKey::from_env().map(|key| key.sign(bytes))
}

pub fn verify(bytes: &[u8], signature: &str) -> Result<bool, SigningError> {
    SigningKey::from_env().map(|key| key.verify(bytes, signature))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameContractVerification {
    pub signature_ok: bool,
    pub time_bounds_ok: bool,
    pub expired: bool,
    pub now_ms: u64,
}

impl GameContractVerification {
    pub fn is_valid(&self) -> bool {
        self.signature_ok && self.time_bounds_ok && !self.expired
    }
}

pub fn sign_game_contract_v1(
    payload: &GameSimulationContractPayloadV1,
) -> Result<GameSimulationContractV1, SigningRequestError> {
    let key = SigningKey::from_env()?;
    sign_game_contract_v1_with_key(payload, &key)
}

pub fn sign_game_contract_v1_with_key(
    payload: &GameSimulationContractPayloadV1,
    key: &SigningKey,
) -> Result<GameSimulationContractV1, SigningRequestError> {
    let bytes = payload.signing_bytes()?;
    Ok(GameSimulationContractV1 {
        payload: payload.clone(),
        signature: key.sign(&bytes),
    })
}

pub fn verify_game_contract_v1(
    contract: &GameSimulationContractV1,
) -> Result<GameContractVerification, SigningRequestError> {
    let key = SigningKey::from_env()?;
    verify_game_contract_v1_with_key(contract, &key)
}

pub fn verify_game_contract_v1_with_key(
    contract: &GameSimulationContractV1,
    key: &SigningKey,
) -> Result<GameContractVerification, SigningRequestError> {
    let now_ms = current_time_ms();
    verify_game_contract_v1_with_key_at(contract, key, now_ms)
}

pub fn verify_game_contract_v1_with_key_at(
    contract: &GameSimulationContractV1,
    key: &SigningKey,
    now_ms: u64,
) -> Result<GameContractVerification, SigningRequestError> {
    let bytes = contract.payload.signing_bytes()?;
    let signature_ok = key.verify(&bytes, &contract.signature);
    let time_bounds_ok = contract.payload.issued_at_ms <= contract.payload.expires_at_ms;
    let expired = now_ms > contract.payload.expires_at_ms;

    Ok(GameContractVerification {
        signature_ok,
        time_bounds_ok,
        expired,
        now_ms,
    })
}

fn current_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::game::v1::{
        GamePlayerTuningV1, GameSimulationContractPayloadV1, GameSimulationRequestV1,
    };

    fn sample_request() -> GameSimulationRequestV1 {
        GameSimulationRequestV1 {
            tuning: GamePlayerTuningV1 {
                aero_points: 8,
                chassis_points: 6,
                engine_points: 12,
                cooling_points: 4,
                downforce_slider: 0.4,
                gear_ratio_slider: 0.6,
            },
            track_id: "demo-oval".to_string(),
            hz: 60.0,
            seed: Some(7),
            engine_version: Some("0.1.0".to_string()),
        }
    }

    #[test]
    fn contract_signature_validates_and_expires() {
        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret");
        let request = sample_request();
        let payload = GameSimulationContractPayloadV1 {
            request,
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_000_500,
            nonce: "unit-test".to_string(),
        };
        let contract = sign_game_contract_v1_with_key(&payload, &key).expect("signed");

        let ok = verify_game_contract_v1_with_key_at(&contract, &key, 1_700_000_000_100)
            .expect("verified");
        assert!(ok.signature_ok);
        assert!(ok.time_bounds_ok);
        assert!(!ok.expired);

        let expired = verify_game_contract_v1_with_key_at(&contract, &key, 1_700_000_000_900)
            .expect("verified");
        assert!(expired.expired);
    }

    #[test]
    fn contract_signature_breaks_on_tamper() {
        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret");
        let request = sample_request();
        let payload = GameSimulationContractPayloadV1 {
            request,
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_000_500,
            nonce: "unit-test".to_string(),
        };
        let mut contract = sign_game_contract_v1_with_key(&payload, &key).expect("signed");
        contract.payload.expires_at_ms += 1;

        let result = verify_game_contract_v1_with_key_at(&contract, &key, 1_700_000_000_100)
            .expect("verified");
        assert!(!result.signature_ok);
    }

    #[test]
    fn contract_boundary_is_not_expired() {
        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret");
        let request = sample_request();
        let payload = GameSimulationContractPayloadV1 {
            request,
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_000_500,
            nonce: "unit-test".to_string(),
        };
        let contract = sign_game_contract_v1_with_key(&payload, &key).expect("signed");

        let result = verify_game_contract_v1_with_key_at(&contract, &key, 1_700_000_000_500)
            .expect("verified");
        assert!(!result.expired);
    }

    #[test]
    fn contract_invalid_time_bounds() {
        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret");
        let request = sample_request();
        let payload = GameSimulationContractPayloadV1 {
            request,
            issued_at_ms: 1_700_000_000_600,
            expires_at_ms: 1_700_000_000_500,
            nonce: "unit-test".to_string(),
        };
        let contract = sign_game_contract_v1_with_key(&payload, &key).expect("signed");

        let result = verify_game_contract_v1_with_key_at(&contract, &key, 1_700_000_000_500)
            .expect("verified");
        assert!(!result.time_bounds_ok);
    }
}

use hmac::{Hmac, Mac};
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

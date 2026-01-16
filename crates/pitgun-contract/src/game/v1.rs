use serde::{Deserialize, Serialize};

pub const DEFAULT_HZ: f32 = 60.0;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct GamePlayerTuningV1 {
    pub aero_points: i32,
    pub chassis_points: i32,
    pub engine_points: i32,
    pub cooling_points: i32,
    pub downforce_slider: f32,
    pub gear_ratio_slider: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameSimulationRequestV1 {
    pub tuning: GamePlayerTuningV1,
    pub track_id: String,
    #[serde(default = "default_hz")]
    pub hz: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameTelemetryPointV1 {
    pub time_s: f32,
    pub s_m: f32,
    pub x_m: f32,
    pub y_m: f32,
    pub heading_rad: f32,
    pub speed_kph: f32,
    pub rpm: f32,
    pub gear: i32,
    pub throttle_pct: f32,
    pub brake_pct: f32,
    pub g_lat: f32,
    pub g_long: f32,
    pub engine_temp_c: f32,
    pub engine_power_w: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameTelemetrySummaryV1 {
    pub lap_time_s: f32,
    pub max_speed_kph: f32,
    pub avg_speed_kph: f32,
    pub max_rpm: f32,
    pub max_g_lat: f32,
    pub max_g_long: f32,
    pub max_engine_temp_c: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameSimulationResultV1 {
    pub lap_time_s: f32,
    pub summary: GameTelemetrySummaryV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<Vec<GameTelemetryPointV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_ref: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameSimulationContractPayloadV1 {
    pub request: GameSimulationRequestV1,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub nonce: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameSimulationContractV1 {
    #[serde(flatten)]
    pub payload: GameSimulationContractPayloadV1,
    pub signature: String,
}

#[derive(Debug)]
pub enum SigningBytesError {
    NonFiniteFloat(&'static str),
}

impl std::fmt::Display for SigningBytesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SigningBytesError::NonFiniteFloat(field) => {
                write!(f, "{field} must be finite")
            }
        }
    }
}

impl std::error::Error for SigningBytesError {}

impl GameSimulationRequestV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, SigningBytesError> {
        let mut out = Vec::with_capacity(128);
        out.extend_from_slice(b"pitgun-game-simreq-v1\0");

        push_string(&mut out, &self.track_id);
        push_f32(&mut out, self.hz, "hz")?;

        let tuning = &self.tuning;
        out.extend_from_slice(&tuning.aero_points.to_le_bytes());
        out.extend_from_slice(&tuning.chassis_points.to_le_bytes());
        out.extend_from_slice(&tuning.engine_points.to_le_bytes());
        out.extend_from_slice(&tuning.cooling_points.to_le_bytes());
        push_f32(&mut out, tuning.downforce_slider, "downforce_slider")?;
        push_f32(&mut out, tuning.gear_ratio_slider, "gear_ratio_slider")?;

        match self.seed {
            Some(value) => {
                out.push(1);
                out.extend_from_slice(&value.to_le_bytes());
            }
            None => out.push(0),
        }

        match &self.engine_version {
            Some(value) => {
                out.push(1);
                push_string(&mut out, value);
            }
            None => out.push(0),
        }

        Ok(out)
    }
}

impl GameSimulationContractPayloadV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, SigningBytesError> {
        let mut out = Vec::with_capacity(192);
        out.extend_from_slice(b"pitgun-game-contract-v1\0");
        out.extend_from_slice(&self.request.signing_bytes()?);
        out.extend_from_slice(&self.issued_at_ms.to_le_bytes());
        out.extend_from_slice(&self.expires_at_ms.to_le_bytes());
        push_string(&mut out, &self.nonce);
        Ok(out)
    }
}

fn default_hz() -> f32 {
    DEFAULT_HZ
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

fn push_f32(out: &mut Vec<u8>, value: f32, field: &'static str) -> Result<(), SigningBytesError> {
    if !value.is_finite() {
        return Err(SigningBytesError::NonFiniteFloat(field));
    }
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_signing::SigningKey;

    #[test]
    fn signing_bytes_are_deterministic() {
        let request = GameSimulationRequestV1 {
            tuning: GamePlayerTuningV1 {
                aero_points: 6,
                chassis_points: 8,
                engine_points: 12,
                cooling_points: 4,
                downforce_slider: 0.4,
                gear_ratio_slider: 0.7,
            },
            track_id: "demo-oval".to_string(),
            hz: 60.0,
            seed: Some(42),
            engine_version: Some("0.1.0".to_string()),
        };

        let bytes_a = request.signing_bytes().expect("signing bytes");
        let bytes_b = request.signing_bytes().expect("signing bytes");
        assert_eq!(bytes_a, bytes_b);

        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid");
        let signature = key.sign(&bytes_a);
        assert!(key.verify(&bytes_b, &signature));
    }

    #[test]
    fn signing_bytes_rejects_nan() {
        let mut request = GameSimulationRequestV1 {
            tuning: GamePlayerTuningV1 {
                aero_points: 0,
                chassis_points: 0,
                engine_points: 0,
                cooling_points: 0,
                downforce_slider: 0.0,
                gear_ratio_slider: 0.0,
            },
            track_id: "demo-oval".to_string(),
            hz: f32::NAN,
            seed: None,
            engine_version: None,
        };

        assert!(request.signing_bytes().is_err());

        request.hz = 60.0;
        request.tuning.downforce_slider = f32::INFINITY;
        assert!(request.signing_bytes().is_err());
    }

    #[test]
    fn contract_signing_bytes_are_deterministic() {
        let request = GameSimulationRequestV1 {
            tuning: GamePlayerTuningV1 {
                aero_points: 6,
                chassis_points: 8,
                engine_points: 12,
                cooling_points: 4,
                downforce_slider: 0.4,
                gear_ratio_slider: 0.7,
            },
            track_id: "demo-oval".to_string(),
            hz: 60.0,
            seed: Some(42),
            engine_version: Some("0.1.0".to_string()),
        };
        let payload = GameSimulationContractPayloadV1 {
            request,
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_600_000,
            nonce: "unit-test".to_string(),
        };

        let bytes_a = payload.signing_bytes().expect("signing bytes");
        let bytes_b = payload.signing_bytes().expect("signing bytes");
        assert_eq!(bytes_a, bytes_b);

        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid");
        let signature = key.sign(&bytes_a);
        assert!(key.verify(&bytes_b, &signature));
    }
}

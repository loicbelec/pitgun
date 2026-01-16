use pitgun_contract::game::v1::{
    GameSimulationContractV1, GameSimulationRequestV1, GameSimulationResultV1,
};

#[derive(Debug)]
pub enum SimulationJsonError {
    InvalidJson(serde_json::Error),
}

impl std::fmt::Display for SimulationJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimulationJsonError::InvalidJson(err) => write!(f, "invalid JSON payload: {err}"),
        }
    }
}

impl std::error::Error for SimulationJsonError {}

impl From<serde_json::Error> for SimulationJsonError {
    fn from(value: serde_json::Error) -> Self {
        SimulationJsonError::InvalidJson(value)
    }
}

pub fn deserialize_game_simulation_request_v1(
    bytes: &[u8],
) -> Result<GameSimulationRequestV1, SimulationJsonError> {
    Ok(serde_json::from_slice(bytes)?)
}

pub fn deserialize_game_simulation_result_v1(
    bytes: &[u8],
) -> Result<GameSimulationResultV1, SimulationJsonError> {
    Ok(serde_json::from_slice(bytes)?)
}

pub fn deserialize_game_simulation_contract_v1(
    bytes: &[u8],
) -> Result<GameSimulationContractV1, SimulationJsonError> {
    Ok(serde_json::from_slice(bytes)?)
}

pub fn serialize_game_simulation_request_v1(
    request: &GameSimulationRequestV1,
) -> Result<Vec<u8>, SimulationJsonError> {
    Ok(serde_json::to_vec(request)?)
}

pub fn serialize_game_simulation_result_v1(
    result: &GameSimulationResultV1,
) -> Result<Vec<u8>, SimulationJsonError> {
    Ok(serde_json::to_vec(result)?)
}

pub fn serialize_game_simulation_contract_v1(
    contract: &GameSimulationContractV1,
) -> Result<Vec<u8>, SimulationJsonError> {
    Ok(serde_json::to_vec(contract)?)
}

pub fn extract_game_simulation_request_v1(
    contract: &GameSimulationContractV1,
) -> &GameSimulationRequestV1 {
    &contract.payload.request
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::game::v1::{
        GamePlayerTuningV1, GameSimulationContractPayloadV1, GameSimulationRequestV1,
    };

    #[test]
    fn round_trips_simulation_request() {
        let request = GameSimulationRequestV1 {
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
        };

        let bytes = serialize_game_simulation_request_v1(&request).expect("serialize");
        let decoded = deserialize_game_simulation_request_v1(&bytes).expect("deserialize");
        assert_eq!(decoded, request);
    }

    #[test]
    fn round_trips_simulation_contract() {
        let request = GameSimulationRequestV1 {
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
        };
        let payload = GameSimulationContractPayloadV1 {
            request,
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_600_000,
            nonce: "unit-test".to_string(),
        };
        let contract = GameSimulationContractV1 {
            payload,
            signature: "deadbeef".to_string(),
        };

        let bytes = serialize_game_simulation_contract_v1(&contract).expect("serialize");
        let decoded = deserialize_game_simulation_contract_v1(&bytes).expect("deserialize");
        assert_eq!(decoded, contract);
        assert_eq!(
            extract_game_simulation_request_v1(&decoded),
            &contract.payload.request
        );
    }
}

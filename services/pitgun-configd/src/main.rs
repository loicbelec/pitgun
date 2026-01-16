use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pitgun_contract::game::v1::{
    GameSimulationContractPayloadV1, GameSimulationContractV1, GameSimulationRequestV1,
};
use pitgun_policy::{PolicyError, TuningPolicyV1};
use pitgun_signing::{SigningKey, sign_game_contract_v1_with_key};
use tokio::net::TcpListener;
use tracing::{error, info, warn};
use uuid::Uuid;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_TTL_MS: u64 = 600_000;

#[derive(Clone)]
struct AppState {
    signing_key: Option<SigningKey>,
    tuning_policy: TuningPolicyV1,
    ttl_ms: u64,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
    details: String,
}

#[derive(Debug)]
enum ContractError {
    BadRequest(String),
    Internal(String),
}

impl ContractError {
    fn into_response(self) -> Response {
        match self {
            ContractError::BadRequest(details) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid request".to_string(),
                    details,
                }),
            )
                .into_response(),
            ContractError::Internal(details) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal error".to_string(),
                    details,
                }),
            )
                .into_response(),
        }
    }
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn readyz(State(state): State<AppState>) -> StatusCode {
    if state.signing_key.is_some() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn deprecated_validate_config() -> (StatusCode, Json<ErrorResponse>) {
    warn!("/v1/config/validate is deprecated");
    (
        StatusCode::GONE,
        Json(ErrorResponse {
            error: "deprecated".to_string(),
            details: "/v1/config/validate has been removed; use /v1/requests/game".to_string(),
        }),
    )
}

async fn create_game_request(
    State(state): State<AppState>,
    Json(request): Json<GameSimulationRequestV1>,
) -> Response {
    match build_signed_game_request(&state, request) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => err.into_response(),
    }
}

fn build_signed_game_request(
    state: &AppState,
    mut request: GameSimulationRequestV1,
) -> Result<GameSimulationContractV1, ContractError> {
    let key = state
        .signing_key
        .as_ref()
        .ok_or_else(|| ContractError::Internal("signing key unavailable".to_string()))?;

    let trimmed_track = request.track_id.trim();
    if trimmed_track.is_empty() {
        return Err(ContractError::BadRequest(
            "track_id must be a non-empty string".to_string(),
        ));
    }
    request.track_id = trimmed_track.to_string();

    if !request.hz.is_finite() || request.hz <= 0.0 {
        return Err(ContractError::BadRequest(
            "hz must be finite and > 0".to_string(),
        ));
    }

    request.tuning = state
        .tuning_policy
        .normalize(request.tuning)
        .map_err(|err| ContractError::BadRequest(policy_error_message(err)))?;

    let issued_at_ms = now_ms();
    let expires_at_ms = issued_at_ms
        .checked_add(state.ttl_ms)
        .ok_or_else(|| ContractError::Internal("contract TTL overflow".to_string()))?;
    let payload = GameSimulationContractPayloadV1 {
        request,
        issued_at_ms,
        expires_at_ms,
        nonce: Uuid::new_v4().to_string(),
    };

    sign_game_contract_v1_with_key(&payload, key)
        .map_err(|err| ContractError::Internal(err.to_string()))
}

fn policy_error_message(err: PolicyError) -> String {
    err.to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let signing_key = match SigningKey::from_env() {
        Ok(key) => Some(key),
        Err(err) => {
            error!(?err, "signing secret unavailable; /readyz will report 503");
            None
        }
    };

    let app_state = AppState {
        signing_key,
        tuning_policy: TuningPolicyV1::default(),
        ttl_ms: read_ttl_ms(),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/config/validate", post(deprecated_validate_config))
        .route("/v1/requests/game", post(create_game_request))
        .with_state(app_state);

    let bind_addr =
        std::env::var("PITGUN_CONFIGD_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
    let addr: SocketAddr = bind_addr
        .parse()
        .map_err(|err| format!("invalid PITGUN_CONFIGD_BIND: {err}"))?;

    let listener = TcpListener::bind(addr).await?;

    info!("pitgun-configd listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

fn read_ttl_ms() -> u64 {
    std::env::var("PITGUN_CONTRACT_TTL_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TTL_MS)
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::game::v1::GamePlayerTuningV1;

    fn test_state() -> AppState {
        AppState {
            signing_key: Some(
                SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid"),
            ),
            tuning_policy: TuningPolicyV1::default(),
            ttl_ms: 600_000,
        }
    }

    fn base_request() -> GameSimulationRequestV1 {
        GameSimulationRequestV1 {
            tuning: GamePlayerTuningV1 {
                aero_points: 10,
                chassis_points: 10,
                engine_points: 10,
                cooling_points: 10,
                downforce_slider: 0.5,
                gear_ratio_slider: 0.5,
            },
            track_id: "demo-oval".to_string(),
            hz: 60.0,
            seed: Some(1),
            engine_version: Some("0.1.0".to_string()),
        }
    }

    #[test]
    fn invalid_points_are_bad_request() {
        let state = test_state();
        let mut request = base_request();
        request.tuning.aero_points = 99;

        let err = build_signed_game_request(&state, request).expect_err("should reject");
        match err {
            ContractError::BadRequest(message) => {
                assert!(message.contains("aero_points"));
            }
            ContractError::Internal(message) => panic!("unexpected internal error: {message}"),
        }
    }

    #[test]
    fn happy_path_returns_signature() {
        let state = test_state();
        let request = base_request();

        let response = build_signed_game_request(&state, request).expect("should succeed");
        assert!(!response.signature.is_empty());
        assert!(response.payload.expires_at_ms >= response.payload.issued_at_ms);

        let bytes = response
            .payload
            .signing_bytes()
            .expect("payload should serialize");
        let key = SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid");
        assert!(key.verify(&bytes, &response.signature));
    }

    #[tokio::test]
    async fn legacy_validate_returns_gone() {
        let (status, Json(body)) = deprecated_validate_config().await;
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(body.error, "deprecated");
        assert_eq!(
            body.details,
            "/v1/config/validate has been removed; use /v1/requests/game"
        );
    }
}

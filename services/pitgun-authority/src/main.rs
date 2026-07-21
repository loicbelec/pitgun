use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::SocketAddr,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pitgun_policy::{
    PlayerTuningRequest, TuningEvalContext, TuningPolicyV1, load_tuning_v1_from_str,
};
use pitgun_racing_contract::{SignedSimulationContractV1, SimulationContractV1};
use pitgun_racing_policy::default_policy_path;
use pitgun_signing::SigningKey;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_SIM_TTL_SECS: u64 = 300;

#[derive(Clone)]
struct AppState {
    signing_key: Option<SigningKey>,
    tuning_policy: TuningPolicyV1,
    policy_hash: String,
    config: ServiceConfig,
}

#[derive(Clone)]
struct ServiceConfig {
    simulation_contract_ttl_secs: u64,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
    details: String,
}

#[derive(serde::Deserialize, Clone)]
struct SimulationContractRequest {
    era: u32,
    category_levels: BTreeMap<String, i64>,
    owned_upgrades: Vec<String>,
    parameters: JsonValue,
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
            details: "/v1/config/validate has been removed; use /v1/contracts/simulation"
                .to_string(),
        }),
    )
}

async fn create_simulation_contract(
    State(state): State<AppState>,
    Json(request): Json<SimulationContractRequest>,
) -> Response {
    match build_signed_simulation_contract(now_ms(), &state, request) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => err.into_response(),
    }
}

fn build_signed_simulation_contract(
    now_ms: i64,
    state: &AppState,
    request: SimulationContractRequest,
) -> Result<SignedSimulationContractV1, ContractError> {
    let signing_key =
        SigningKey::from_env().map_err(|err| ContractError::Internal(err.to_string()))?;

    let mut category_levels = BTreeMap::new();
    for (key, value) in request.category_levels {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            return Err(ContractError::BadRequest(
                "category_levels keys must be non-empty strings".to_string(),
            ));
        }
        category_levels.insert(trimmed.to_string(), value);
    }

    let mut owned_upgrades = BTreeSet::new();
    for upgrade in request.owned_upgrades {
        let trimmed = upgrade.trim();
        if trimmed.is_empty() {
            continue;
        }
        owned_upgrades.insert(trimmed.to_string());
    }

    let ctx = TuningEvalContext {
        era: request.era,
        category_levels: category_levels.clone(),
        owned_upgrades: owned_upgrades.clone(),
    };
    let player_request = PlayerTuningRequest {
        parameters: request.parameters,
    };

    let canonical = state
        .tuning_policy
        .canonicalize(&ctx, &player_request)
        .map_err(|err| ContractError::BadRequest(err.to_string()))?;
    state
        .tuning_policy
        .validate_constraints(&ctx, &canonical)
        .map_err(|err| ContractError::BadRequest(err.to_string()))?;

    let derived_constraints = state
        .tuning_policy
        .derived_constraints
        .as_ref()
        .map(|constraints| {
            let mut names: Vec<String> = constraints.iter().map(|item| item.name.clone()).collect();
            names.sort();
            names
        })
        .filter(|names| !names.is_empty());

    let issued_at_ms = now_ms;
    let ttl_ms = (state
        .config
        .simulation_contract_ttl_secs
        .saturating_mul(1_000)) as i64;
    let contract = SimulationContractV1 {
        version: "SimulationContractV1".to_string(),
        policy_hash: state.policy_hash.clone(),
        issued_at_ms,
        expires_at_ms: issued_at_ms.saturating_add(ttl_ms),
        era: request.era,
        category_levels,
        owned_upgrades: owned_upgrades.into_iter().collect(),
        parameters: canonical.parameters,
        derived_constraints,
    };

    let bytes = contract.signing_bytes().map_err(|err| {
        error!(?err, "failed to serialize simulation contract payload");
        ContractError::Internal("failed to serialize contract payload".to_string())
    })?;
    let signature = signing_key.sign(&bytes);

    Ok(SignedSimulationContractV1 {
        contract,
        signature,
    })
}

fn now_ms() -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}

fn load_config() -> ServiceConfig {
    ServiceConfig {
        simulation_contract_ttl_secs: parse_env_u64(
            "PITGUN_SIM_CONTRACT_TTL_SECONDS",
            DEFAULT_SIM_TTL_SECS,
        ),
    }
}

fn parse_env_u64(var: &str, default: u64) -> u64 {
    std::env::var(var)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn load_tuning_policy(path: PathBuf) -> Result<(TuningPolicyV1, String), String> {
    let bytes = fs::read(&path).map_err(|err| format!("failed to read policy: {err}"))?;
    let policy_hash = sha256_hex(&bytes);
    let contents =
        String::from_utf8(bytes).map_err(|err| format!("policy must be valid UTF-8: {err}"))?;
    let policy =
        load_tuning_v1_from_str(&contents).map_err(|err| format!("invalid policy: {err}"))?;
    policy
        .validate_static()
        .map_err(|err| format!("policy validation failed: {err}"))?;
    Ok((policy, policy_hash))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let config = load_config();
    let signing_key = match SigningKey::from_env() {
        Ok(key) => Some(key),
        Err(err) => {
            error!(?err, "signing secret unavailable; /readyz will report 503");
            None
        }
    };
    let policy_path = default_policy_path();
    let (tuning_policy, policy_hash) = load_tuning_policy(policy_path.clone()).map_err(|err| {
        format!(
            "failed to load game tuning policy at {}: {err}",
            policy_path.display()
        )
    })?;

    let app_state = AppState {
        signing_key,
        tuning_policy,
        policy_hash,
        config,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/config/validate", post(deprecated_validate_config))
        .route("/v1/contracts/simulation", post(create_simulation_contract))
        .with_state(app_state);

    let bind_addr =
        std::env::var("PITGUN_AUTHORITY_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
    let addr: SocketAddr = bind_addr
        .parse()
        .map_err(|err| format!("invalid PITGUN_CONFIGD_BIND: {err}"))?;

    let listener = TcpListener::bind(addr).await?;

    info!("pitgun-authority listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_signing::SIGNING_SECRET_ENV;
    use serde_json::json;
    use std::sync::Mutex;

    fn test_state() -> AppState {
        let policy_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../policies/gametuning.v1.yaml");
        let (tuning_policy, policy_hash) =
            load_tuning_policy(policy_path).expect("policy should load");

        AppState {
            signing_key: Some(
                SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid"),
            ),
            tuning_policy,
            policy_hash,
            config: ServiceConfig {
                simulation_contract_ttl_secs: 300,
            },
        }
    }

    fn base_request(parameters: JsonValue) -> SimulationContractRequest {
        SimulationContractRequest {
            era: 3,
            category_levels: BTreeMap::from([("budget_lvl".to_string(), 100)]),
            owned_upgrades: Vec::new(),
            parameters,
        }
    }

    fn with_signing_env<T>(secret: &str, action: impl FnOnce() -> T) -> T {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = std::env::var(SIGNING_SECRET_ENV).ok();
        unsafe {
            std::env::set_var(SIGNING_SECRET_ENV, secret);
        }
        let result = action();
        unsafe {
            match previous {
                Some(value) => std::env::set_var(SIGNING_SECRET_ENV, value),
                None => std::env::remove_var(SIGNING_SECRET_ENV),
            }
        }
        result
    }

    #[test]
    fn unlock_rejection_is_bad_request() {
        let state = test_state();
        let mut request = base_request(json!({
            "gameplay": {
                "engine_points": 10.0
            }
        }));
        request.era = 0;

        let err = with_signing_env("unit-test-secret", || {
            build_signed_simulation_contract(1_710_000_000_000, &state, request)
                .expect_err("should reject")
        });
        match err {
            ContractError::BadRequest(message) => {
                assert!(message.contains("unlock condition not met"));
            }
            ContractError::Internal(message) => panic!("unexpected internal error: {message}"),
        }
    }

    #[test]
    fn constraint_violation_is_bad_request() {
        let state = test_state();
        let request = base_request(json!({
            "gameplay": {
                "aero_points": 30.0,
                "chassis_points": 30.0,
                "cooling_points": 30.0,
                "engine_points": 30.0
            }
        }));

        let err = with_signing_env("unit-test-secret", || {
            build_signed_simulation_contract(1_710_000_000_000, &state, request)
                .expect_err("should reject")
        });
        match err {
            ContractError::BadRequest(message) => {
                assert!(message.contains("Gameplay setup exceeds available budget."));
            }
            ContractError::Internal(message) => panic!("unexpected internal error: {message}"),
        }
    }

    #[test]
    fn happy_path_returns_signature() {
        let state = test_state();
        let request = base_request(json!({
            "gameplay": {
                "aero_points": 25.0,
                "chassis_points": 25.0,
                "cooling_points": 25.0,
                "engine_points": 25.0,
                "downforce_slider": 0.5,
                "gear_ratio_slider": 0.5
            }
        }));

        let response = with_signing_env("unit-test-secret", || {
            build_signed_simulation_contract(1_710_000_000_000, &state, request)
                .expect("should succeed")
        });
        assert!(!response.signature.is_empty());
        assert_eq!(response.contract.version, "SimulationContractV1");
        assert_eq!(response.contract.policy_hash, state.policy_hash);
        let bytes = response
            .contract
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
            "/v1/config/validate has been removed; use /v1/contracts/simulation"
        );
    }
}

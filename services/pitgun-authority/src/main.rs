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
use pitgun_contract::{
    ArtifactIdentity, AuthorizationSignatureAlgorithm, AuthorizationValidityV1,
    CatalogReleaseIdentityV1, ContractVersion, DeterministicRunContractV1, Digest, EventOrderingV1,
    Identifier, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
    RandomAlgorithm, RandomContractV1, RunAuthorizationV1, RunAuthorizationVersion, RuntimeProfile,
    ScenarioIdentity, Seed, SignedRunAuthorizationV1, StreamDerivation, canonical_json_digest,
};
use pitgun_policy::{
    PlayerTuningRequest, TuningEvalContext, TuningPolicyV1, load_tuning_v1_from_str,
};
use pitgun_racing_contract::{SignedSimulationContractV1, SimulationContractV1};
use pitgun_racing_policy::{default_policy_path, normalize_and_validate_race_input_with_policy};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceInput, racing_model_identity_for_version,
};
use pitgun_signing::SigningKey;
use rand::{RngCore, rngs::OsRng};
use serde_json::Value as JsonValue;
use sha2::{Digest as ShaDigest, Sha256};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_SIM_TTL_SECS: u64 = 300;
const DEFAULT_LATE_SUBMISSION_GRACE_SECS: u64 = 900;
const DEFAULT_SIGNING_KEY_ID: &str = "pitgun-authority-v1";
const DEFAULT_AUDIENCE: &str = "pitgun.verifier";
const DEFAULT_RACING_MODEL_VERSION: &str = "1.0.0";

#[derive(Clone)]
struct AppState {
    signing_key: Option<SigningKey>,
    tuning_policy: TuningPolicyV1,
    policy_hash: String,
    policy_identity: ArtifactIdentity,
    racing_model: ArtifactIdentity,
    racing_catalog: Option<RacingCatalogSnapshot>,
    config: ServiceConfig,
}

#[derive(Clone)]
struct ServiceConfig {
    simulation_contract_ttl_secs: u64,
    late_submission_grace_secs: u64,
    allow_catalog_free: bool,
    signing_key_id: Identifier,
    audience: Identifier,
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

#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RacingRunAuthorizationRequestV1 {
    subject: Identifier,
    seed: Seed,
    input: RunRaceInput,
    #[serde(default)]
    catalog_release: Option<CatalogReleaseIdentityV1>,
    #[serde(default)]
    data_pack: Option<ArtifactIdentity>,
}

#[derive(Debug, serde::Serialize)]
struct RacingRunAuthorizationResponseV1 {
    signed: SignedRunAuthorizationV1,
    canonical_input: RunRaceInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog_release: Option<CatalogReleaseIdentityV1>,
}

#[derive(Debug)]
enum ContractError {
    BadRequest(String),
    Forbidden(String),
    Unavailable(String),
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
            ContractError::Forbidden(details) => (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "forbidden".to_string(),
                    details,
                }),
            )
                .into_response(),
            ContractError::Unavailable(details) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "unavailable".to_string(),
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

async fn create_racing_run_authorization(
    State(state): State<AppState>,
    Json(request): Json<RacingRunAuthorizationRequestV1>,
) -> Response {
    match build_signed_racing_run_authorization(now_ms(), &state, request) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => error.into_response(),
    }
}

fn build_signed_racing_run_authorization(
    now_ms: i64,
    state: &AppState,
    request: RacingRunAuthorizationRequestV1,
) -> Result<RacingRunAuthorizationResponseV1, ContractError> {
    let signing_key = state.signing_key.as_ref().ok_or_else(|| {
        ContractError::Unavailable("authority signing material is unavailable".to_string())
    })?;
    let era = u32::try_from(request.input.era)
        .map_err(|_| ContractError::BadRequest("input.era must be non-negative".to_string()))?;
    let hz = request.input.hz;
    if !hz.is_finite() || hz <= 0.0 || hz.fract() != 0.0 || hz > 1_000_000.0 {
        return Err(ContractError::BadRequest(
            "input.hz must be an integer in the range 1..=1000000".to_string(),
        ));
    }

    let mut canonical_input = request.input;
    canonical_input.race = normalize_and_validate_race_input_with_policy(
        &canonical_input.race,
        era,
        &state.tuning_policy,
    )
    .map_err(|error| ContractError::BadRequest(error.to_string()))?;
    let input_digest = canonical_json_digest(&canonical_input)
        .map_err(|error| ContractError::BadRequest(format!("invalid canonical input: {error}")))?;

    let (data_pack, catalog_release) =
        resolve_data_pack(state, request.catalog_release, request.data_pack)?;
    let hz = hz as u64;
    let divisor = greatest_common_divisor(1_000_000, hz);
    let contract = DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: ScenarioIdentity {
            id: "racing.race"
                .parse()
                .expect("static Racing scenario identifier"),
            version: "1.0.0".parse().expect("static Racing scenario version"),
        },
        model: state.racing_model.clone(),
        data_pack,
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed: request.seed,
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(0, 1_000_000 / divisor, hz / divisor)
            .map_err(|error| ContractError::BadRequest(error.to_string()))?,
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: input_digest,
        },
    };
    if let (Some(catalog), Some(_)) = (&state.racing_catalog, &catalog_release) {
        catalog
            .manifest()
            .validate_for_run(&contract)
            .map_err(|error| ContractError::BadRequest(error.to_string()))?;
    }

    let run_id = contract.run_id().map_err(|error| {
        error!(?error, "failed to derive deterministic run identity");
        ContractError::Internal("failed to derive deterministic run identity".to_string())
    })?;
    let validity = AuthorizationValidityV1 {
        issued_at_ms: now_ms,
        expires_at_ms: add_seconds(now_ms, state.config.simulation_contract_ttl_secs)?,
        late_submission_grace_ms: milliseconds(state.config.late_submission_grace_secs)?,
    };
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let authorization = RunAuthorizationV1 {
        authorization_version: RunAuthorizationVersion::V1,
        nonce: Digest::from_bytes(&nonce),
        subject: request.subject,
        audience: state.config.audience.clone(),
        contract,
        run_id,
        policy: state.policy_identity.clone(),
        signing_key_id: state.config.signing_key_id.clone(),
        validity,
    };
    let signing_bytes = authorization.signing_bytes().map_err(|error| {
        error!(?error, "failed to canonicalize run authorization");
        ContractError::Internal("failed to canonicalize run authorization".to_string())
    })?;
    let signed = SignedRunAuthorizationV1 {
        authorization,
        algorithm: AuthorizationSignatureAlgorithm::HmacSha256,
        signature: signing_key.sign(&signing_bytes),
    };

    Ok(RacingRunAuthorizationResponseV1 {
        signed,
        canonical_input,
        catalog_release,
    })
}

fn resolve_data_pack(
    state: &AppState,
    requested_release: Option<CatalogReleaseIdentityV1>,
    requested_data_pack: Option<ArtifactIdentity>,
) -> Result<(ArtifactIdentity, Option<CatalogReleaseIdentityV1>), ContractError> {
    match (requested_release, requested_data_pack) {
        (Some(release), None) => {
            let catalog = state.racing_catalog.as_ref().ok_or_else(|| {
                ContractError::Unavailable(
                    "catalog-backed issuance is unavailable on this authority".to_string(),
                )
            })?;
            catalog
                .manifest()
                .verify_release_identity(&release)
                .map_err(|error| ContractError::BadRequest(error.to_string()))?;
            Ok((
                catalog.manifest().simulation_pack.identity.clone(),
                Some(release),
            ))
        }
        (None, Some(data_pack)) if state.config.allow_catalog_free => Ok((data_pack, None)),
        (None, Some(_)) => Err(ContractError::Forbidden(
            "catalog-free issuance is disabled by authority policy".to_string(),
        )),
        (Some(_), Some(_)) => Err(ContractError::BadRequest(
            "catalog_release and data_pack are mutually exclusive".to_string(),
        )),
        (None, None) => Err(ContractError::BadRequest(
            "catalog_release or data_pack is required".to_string(),
        )),
    }
}

fn milliseconds(seconds: u64) -> Result<i64, ContractError> {
    seconds
        .checked_mul(1_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| ContractError::Internal("configured duration is too large".to_string()))
}

fn add_seconds(now_ms: i64, seconds: u64) -> Result<i64, ContractError> {
    now_ms.checked_add(milliseconds(seconds)?).ok_or_else(|| {
        ContractError::Internal("configured validity deadline overflowed".to_string())
    })
}

const fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn build_signed_simulation_contract(
    now_ms: i64,
    state: &AppState,
    request: SimulationContractRequest,
) -> Result<SignedSimulationContractV1, ContractError> {
    let signing_key = state.signing_key.as_ref().ok_or_else(|| {
        ContractError::Unavailable("authority signing material is unavailable".to_string())
    })?;

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
        late_submission_grace_secs: parse_env_u64(
            "PITGUN_LATE_SUBMISSION_GRACE_SECONDS",
            DEFAULT_LATE_SUBMISSION_GRACE_SECS,
        ),
        allow_catalog_free: parse_env_bool("PITGUN_ALLOW_CATALOG_FREE", false),
        signing_key_id: parse_env_identifier("PITGUN_SIGNING_KEY_ID", DEFAULT_SIGNING_KEY_ID),
        audience: parse_env_identifier("PITGUN_AUTHORITY_AUDIENCE", DEFAULT_AUDIENCE),
    }
}

fn parse_env_bool(var: &str, default: bool) -> bool {
    std::env::var(var)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn parse_env_identifier(var: &str, default: &str) -> Identifier {
    let value = std::env::var(var).unwrap_or_else(|_| default.to_string());
    value
        .parse()
        .unwrap_or_else(|error| panic!("{var} is invalid: {error}"))
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

fn load_racing_catalog() -> Result<Option<RacingCatalogSnapshot>, String> {
    let Some(path) = std::env::var_os("PITGUN_RACING_CATALOG_RELEASE_DIR") else {
        return Ok(None);
    };
    RacingCatalogSnapshot::from_release_dir(PathBuf::from(path))
        .map(Some)
        .map_err(|error| format!("failed to load Racing catalog release: {error}"))
}

fn load_racing_model() -> Result<ArtifactIdentity, String> {
    let version = std::env::var("PITGUN_RACING_MODEL_VERSION")
        .unwrap_or_else(|_| DEFAULT_RACING_MODEL_VERSION.to_owned());
    racing_model_identity_for_version(&version)
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
    let signing_key = match SigningKey::from_env_or_file() {
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
    let policy_bytes = fs::read(&policy_path)?;
    let policy_identity = ArtifactIdentity {
        id: "pitgun.racing.tuning"
            .parse()
            .expect("static Racing policy identifier"),
        version: "1.0.0".parse().expect("static Racing policy version"),
        digest: Digest::from_bytes(&policy_bytes),
    };
    let racing_model = load_racing_model()?;
    let racing_catalog = load_racing_catalog()?;
    if let Some(catalog) = &racing_catalog {
        catalog
            .manifest()
            .compatibility
            .validate_for(&racing_model, ContractVersion::V1)
            .map_err(|error| format!("configured Racing model/catalog pair is invalid: {error}"))?;
    }

    let app_state = AppState {
        signing_key,
        tuning_policy,
        policy_hash,
        policy_identity,
        racing_model,
        racing_catalog,
        config,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/config/validate", post(deprecated_validate_config))
        .route("/v1/contracts/simulation", post(create_simulation_contract))
        .route(
            "/v1/authorizations/racing",
            post(create_racing_run_authorization),
        )
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
    use pitgun_racing_contract::{CompetitorSpec, RaceInput, TuningSpec};
    use pitgun_racing_simulator::PitStrategyConfig;
    use serde_json::json;
    use std::collections::HashMap;

    fn test_state() -> AppState {
        test_state_for("1.0.0", "v1.0.0")
    }

    fn test_state_for(model_version: &str, catalog_version: &str) -> AppState {
        let policy_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../policies/gametuning.v1.yaml");
        let policy_bytes = fs::read(&policy_path).expect("policy bytes");
        let (tuning_policy, policy_hash) =
            load_tuning_policy(policy_path).expect("policy should load");
        let catalog_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../catalogs/racing")
            .join(catalog_version);

        AppState {
            signing_key: Some(
                SigningKey::from_secret(b"unit-test-secret").expect("secret should be valid"),
            ),
            tuning_policy,
            policy_hash,
            policy_identity: ArtifactIdentity {
                id: "pitgun.racing.tuning".parse().expect("policy id"),
                version: "1.0.0".parse().expect("policy version"),
                digest: Digest::from_bytes(&policy_bytes),
            },
            racing_model: racing_model_identity_for_version(model_version)
                .expect("supported test model"),
            racing_catalog: Some(
                RacingCatalogSnapshot::from_release_dir(catalog_path)
                    .expect("Racing catalog should load"),
            ),
            config: ServiceConfig {
                simulation_contract_ttl_secs: 300,
                late_submission_grace_secs: 900,
                allow_catalog_free: true,
                signing_key_id: "staging-2026-07".parse().expect("key id"),
                audience: "pitgun.verifier".parse().expect("audience"),
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

    fn racing_request(state: &AppState) -> RacingRunAuthorizationRequestV1 {
        RacingRunAuthorizationRequestV1 {
            subject: "398105f6-2f4b-4874-9a1f-0f3aa1ee9d05"
                .parse()
                .expect("subject"),
            seed: Seed::new(42),
            input: RunRaceInput {
                race: RaceInput {
                    track_id: "LASVEGAS".to_string(),
                    laps: 50,
                    competitors: vec![CompetitorSpec {
                        id: "player".to_string(),
                        driver_id: Some("default".to_string()),
                        name: "Player".to_string(),
                        team_id: "team".to_string(),
                        is_player: true,
                        tuning: TuningSpec {
                            engine_points: 25.0,
                            cooling_points: 25.0,
                            aero_points: 25.0,
                            chassis_points: 25.0,
                            downforce_slider: 0.5,
                            gear_ratio_slider: 0.5,
                        },
                        budget_cap: 100.0,
                        stint_strategy: None,
                    }],
                },
                vehicle_id: Some("f1_2026".to_string()),
                pit_strategy: Some(PitStrategyConfig {
                    player_pit_laps: vec![25],
                    pit_loss_ms: None,
                }),
                track_profile: None,
                competitor_profiles: HashMap::new(),
                era: 6,
                hz: 20.0,
            },
            catalog_release: Some(
                state
                    .racing_catalog
                    .as_ref()
                    .expect("catalog")
                    .release_identity()
                    .clone(),
            ),
            data_pack: None,
        }
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

        let err = build_signed_simulation_contract(1_710_000_000_000, &state, request)
            .expect_err("should reject");
        match err {
            ContractError::BadRequest(message) => {
                assert!(message.contains("unlock condition not met"));
            }
            ContractError::Forbidden(message) => panic!("unexpected forbidden error: {message}"),
            ContractError::Unavailable(message) => {
                panic!("unexpected unavailable error: {message}")
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

        let err = build_signed_simulation_contract(1_710_000_000_000, &state, request)
            .expect_err("should reject");
        match err {
            ContractError::BadRequest(message) => {
                assert!(message.contains("Gameplay setup exceeds available budget."));
            }
            ContractError::Forbidden(message) => panic!("unexpected forbidden error: {message}"),
            ContractError::Unavailable(message) => {
                panic!("unexpected unavailable error: {message}")
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

        let response = build_signed_simulation_contract(1_710_000_000_000, &state, request)
            .expect("should succeed");
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

    #[test]
    fn racing_authorization_binds_catalog_policy_and_canonical_input() {
        let state = test_state();
        let response = build_signed_racing_run_authorization(
            1_710_000_000_000,
            &state,
            racing_request(&state),
        )
        .expect("Racing authorization");
        let authorization = &response.signed.authorization;
        let catalog = state.racing_catalog.as_ref().expect("catalog");

        assert_eq!(
            authorization.contract.data_pack,
            catalog.manifest().simulation_pack.identity
        );
        assert_eq!(authorization.policy, state.policy_identity);
        assert_eq!(
            authorization.contract.input.digest,
            canonical_json_digest(&response.canonical_input).expect("canonical input")
        );
        assert_eq!(
            response.catalog_release.as_ref(),
            Some(catalog.release_identity())
        );
        let bytes = authorization.signing_bytes().expect("signing bytes");
        let key = SigningKey::from_secret(b"unit-test-secret").expect("key");
        assert!(key.verify(&bytes, &response.signed.signature));
    }

    #[test]
    fn racing_v2_authorization_binds_the_exact_model_and_catalog() {
        let state = test_state_for("2.0.0", "v1.2.0");
        let response = build_signed_racing_run_authorization(
            1_710_000_000_000,
            &state,
            racing_request(&state),
        )
        .expect("Racing V2 authorization");

        assert_eq!(
            response.signed.authorization.contract.model,
            state.racing_model
        );
        assert_eq!(
            response.signed.authorization.contract.data_pack,
            state
                .racing_catalog
                .as_ref()
                .expect("catalog")
                .manifest()
                .simulation_pack
                .identity
        );
    }

    #[test]
    fn identical_semantic_requests_share_run_id_but_not_nonce() {
        let state = test_state();
        let first = build_signed_racing_run_authorization(
            1_710_000_000_000,
            &state,
            racing_request(&state),
        )
        .expect("first authorization");
        let second = build_signed_racing_run_authorization(
            1_710_000_000_000,
            &state,
            racing_request(&state),
        )
        .expect("second authorization");

        assert_eq!(
            first.signed.authorization.run_id,
            second.signed.authorization.run_id
        );
        assert_ne!(
            first.signed.authorization.nonce,
            second.signed.authorization.nonce
        );
    }

    #[test]
    fn catalog_free_issuance_uses_the_explicit_data_pack() {
        let state = test_state();
        let mut request = racing_request(&state);
        request.catalog_release = None;
        request.data_pack = Some(ArtifactIdentity {
            id: "customer.private-pack".parse().expect("pack id"),
            version: "2.1.0".parse().expect("pack version"),
            digest: Digest::from_bytes(b"private data pack"),
        });

        let response = build_signed_racing_run_authorization(1_710_000_000_000, &state, request)
            .expect("catalog-free authorization");

        assert_eq!(
            response
                .signed
                .authorization
                .contract
                .data_pack
                .id
                .to_string(),
            "customer.private-pack"
        );
        assert!(response.catalog_release.is_none());
    }

    #[test]
    fn malformed_catalog_selection_fails_closed() {
        let state = test_state();
        let mut request = racing_request(&state);
        request.data_pack = Some(ArtifactIdentity {
            id: "customer.private-pack".parse().expect("pack id"),
            version: "1.0.0".parse().expect("pack version"),
            digest: Digest::from_bytes(b"private data pack"),
        });

        let error = build_signed_racing_run_authorization(1_710_000_000_000, &state, request)
            .expect_err("ambiguous selection must fail");
        assert!(matches!(error, ContractError::BadRequest(_)));
    }

    #[test]
    fn catalog_free_issuance_requires_server_authorization() {
        let mut state = test_state();
        state.config.allow_catalog_free = false;
        let mut request = racing_request(&state);
        request.catalog_release = None;
        request.data_pack = Some(ArtifactIdentity {
            id: "customer.private-pack".parse().expect("pack id"),
            version: "1.0.0".parse().expect("pack version"),
            digest: Digest::from_bytes(b"private data pack"),
        });

        let error = build_signed_racing_run_authorization(1_710_000_000_000, &state, request)
            .expect_err("disabled catalog-free issuance must fail");
        assert!(matches!(error, ContractError::Forbidden(_)));
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

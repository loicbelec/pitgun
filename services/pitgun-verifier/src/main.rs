use std::{
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
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
    ArtifactIdentity, Digest, Identifier, SemanticVersion, VerificationStatus,
    VerificationVerdictV1,
};
use pitgun_racing_policy::default_policy_path;
use pitgun_racing_simulator::RacingCatalogSnapshot;
use pitgun_signing::{SigningKey, VerificationKeyring};
use pitgun_verifier::{RacingVerificationSubmissionV1, RacingVerifier};
use tokio::{net::TcpListener, sync::Semaphore, task};
use tracing::{error, info, warn};

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_AUDIENCE: &str = "pitgun.verifier";
const DEFAULT_SIGNING_KEY_ID: &str = "pitgun-authority-v1";
const DEFAULT_MAX_CONCURRENT_REPLAYS: usize = 2;

#[derive(Clone)]
struct AppState {
    verifier: Arc<RacingVerifier>,
    replay_slots: Arc<Semaphore>,
    ready: bool,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: &'static str,
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn readyz(State(state): State<AppState>) -> StatusCode {
    if state.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn verify_racing(
    State(state): State<AppState>,
    Json(submission): Json<RacingVerificationSubmissionV1>,
) -> Response {
    let permit = match state.replay_slots.clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(error) => {
            error!(?error, "verifier replay semaphore is closed");
            return internal_error();
        }
    };
    let verifier = Arc::clone(&state.verifier);
    let now_ms = now_ms();
    let result = task::spawn_blocking(move || {
        let _permit = permit;
        verifier.verify(&submission, now_ms)
    })
    .await;

    match result {
        Ok(Ok(verdict)) => verdict_response(verdict),
        Ok(Err(error)) => {
            error!(?error, "Racing verification failed internally");
            internal_error()
        }
        Err(error) => {
            error!(?error, "Racing verification worker terminated");
            internal_error()
        }
    }
}

fn verdict_response(verdict: VerificationVerdictV1) -> Response {
    let status = match verdict.status {
        VerificationStatus::Pending => StatusCode::ACCEPTED,
        VerificationStatus::Verified | VerificationStatus::Rejected => StatusCode::OK,
    };
    (status, Json(verdict)).into_response()
}

fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "verification unavailable",
        }),
    )
        .into_response()
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/verifications/racing", post(verify_racing))
        .with_state(state)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn parse_identifier(var: &str, default: &str) -> Identifier {
    std::env::var(var)
        .unwrap_or_else(|_| default.to_owned())
        .parse()
        .unwrap_or_else(|error| panic!("{var} is invalid: {error}"))
}

fn parse_max_concurrent_replays() -> usize {
    let value = std::env::var("PITGUN_VERIFIER_MAX_CONCURRENT_REPLAYS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_MAX_CONCURRENT_REPLAYS);
    assert!(
        value > 0,
        "PITGUN_VERIFIER_MAX_CONCURRENT_REPLAYS must be greater than zero"
    );
    value
}

fn load_policy_identity() -> Result<ArtifactIdentity, String> {
    let path = default_policy_path();
    let bytes = fs::read(&path)
        .map_err(|error| format!("cannot read policy {}: {error}", path.display()))?;
    Ok(ArtifactIdentity {
        id: "pitgun.racing.tuning"
            .parse()
            .expect("static Racing policy identifier"),
        version: "1.0.0".parse().expect("static Racing policy version"),
        digest: Digest::from_bytes(&bytes),
    })
}

fn load_catalog() -> Option<RacingCatalogSnapshot> {
    let loaded = match std::env::var_os("PITGUN_RACING_CATALOG_RELEASE_DIR") {
        Some(path) => RacingCatalogSnapshot::from_release_dir(PathBuf::from(path)),
        None => RacingCatalogSnapshot::embedded(),
    };
    match loaded {
        Ok(catalog) => Some(catalog),
        Err(error) => {
            error!(?error, "retained Racing catalog is unavailable");
            None
        }
    }
}

fn load_verifier_identity() -> Result<ArtifactIdentity, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("cannot locate verifier: {error}"))?;
    let bytes = fs::read(&executable).map_err(|error| {
        format!(
            "cannot read verifier executable {}: {error}",
            executable.display()
        )
    })?;
    Ok(ArtifactIdentity {
        id: "pitgun.verifier"
            .parse()
            .expect("static verifier identifier"),
        version: SemanticVersion::new(env!("CARGO_PKG_VERSION"))
            .expect("package version is semantic"),
        digest: Digest::from_bytes(&bytes),
    })
}

fn load_state() -> Result<AppState, String> {
    let key_id = parse_identifier("PITGUN_SIGNING_KEY_ID", DEFAULT_SIGNING_KEY_ID);
    let expected_audience = parse_identifier("PITGUN_VERIFIER_AUDIENCE", DEFAULT_AUDIENCE);
    let signing_key = match SigningKey::from_env_or_file() {
        Ok(key) => Some(key),
        Err(error) => {
            error!(?error, "Authority verification material is unavailable");
            None
        }
    };
    let mut keyring = VerificationKeyring::new();
    if let Some(key) = signing_key.clone() {
        keyring.insert(key_id, key);
    }
    let policy = load_policy_identity()?;
    let catalog = load_catalog();
    let ready = signing_key.is_some() && catalog.is_some();
    let verifier = RacingVerifier::new(
        keyring,
        expected_audience,
        policy,
        catalog,
        load_verifier_identity()?,
    );

    Ok(AppState {
        verifier: Arc::new(verifier),
        replay_slots: Arc::new(Semaphore::new(parse_max_concurrent_replays())),
        ready,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let state = load_state()?;
    if !state.ready {
        warn!("pitgun-verifier started unready and will fail closed");
    }
    let bind_addr =
        std::env::var("PITGUN_VERIFIER_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());
    let address: SocketAddr = bind_addr
        .parse()
        .map_err(|error| format!("invalid PITGUN_VERIFIER_BIND: {error}"))?;
    let listener = TcpListener::bind(address).await?;
    info!("pitgun-verifier listening on {}", listener.local_addr()?);
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pitgun_contract::{
        ArtifactIdentity, Digest, ExecutionId, SubmittedEvidenceV1, VerificationReasonCode,
        VerificationStatus, VerificationVerdictV1, VerificationVerdictVersion,
    };

    use super::verdict_response;

    fn verdict(status: VerificationStatus) -> VerificationVerdictV1 {
        VerificationVerdictV1 {
            schema_version: VerificationVerdictVersion::V1,
            run_id: Digest::from_bytes(b"run"),
            execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"
                .parse::<ExecutionId>()
                .expect("execution id"),
            status,
            reason_code: (status == VerificationStatus::Pending)
                .then_some(VerificationReasonCode::VerificationQueued),
            submitted_evidence: SubmittedEvidenceV1 {
                receipt_digest: Digest::from_bytes(b"receipt"),
                output_digest: Digest::from_bytes(b"output"),
                telemetry_summary_digest: Digest::from_bytes(b"telemetry"),
            },
            verified_resolution: None,
            verifier: ArtifactIdentity {
                id: "pitgun.verifier".parse().expect("verifier id"),
                version: "1.0.0".parse().expect("verifier version"),
                digest: Digest::from_bytes(b"verifier"),
            },
            recorded_at_ms: 1,
        }
    }

    #[test]
    fn pending_is_accepted_and_terminal_decisions_are_ok() {
        assert_eq!(
            verdict_response(verdict(VerificationStatus::Pending)).status(),
            axum::http::StatusCode::ACCEPTED
        );

        let mut rejected = verdict(VerificationStatus::Rejected);
        rejected.reason_code = Some(VerificationReasonCode::ReplayMismatch);
        assert_eq!(
            verdict_response(rejected).status(),
            axum::http::StatusCode::OK
        );
    }
}

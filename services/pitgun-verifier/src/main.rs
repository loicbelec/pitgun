mod service_replay_config;

use std::{
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pitgun_contract::{
    ArtifactIdentity, CatalogReleaseIdentityV1, Digest, Identifier, SemanticVersion,
    VerificationStatus, VerificationVerdictV1,
};
use pitgun_racing_policy::default_policy_path;
use pitgun_racing_simulator::{RacingCatalogSnapshot, racing_model_identity_for_version};
use pitgun_signing::{SigningKey, VerificationKeyring};
use pitgun_verifier::{
    RacingDynamicVerificationSubmissionV1, RacingServiceVerificationSubmissionV1,
    RacingVerificationSubmissionV1, RacingVerifier,
};
use tokio::{
    net::TcpListener,
    sync::{AcquireError, OwnedSemaphorePermit, Semaphore},
    task,
};
use tracing::{error, info, warn};

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_AUDIENCE: &str = "pitgun.verifier";
const DEFAULT_SIGNING_KEY_ID: &str = "pitgun-authority-v1";
const DEFAULT_MAX_CONCURRENT_REPLAYS: usize = 2;
const DEFAULT_RACING_MODEL_VERSION: &str = "1.0.0";
const DEFAULT_RACING_ATTEMPT_MODEL_VERSION: &str = "0.14.0";

#[derive(Clone)]
struct AppState {
    verifier: Arc<RacingVerifier>,
    attempt_verifier: Arc<RacingVerifier>,
    retained_verifiers: Vec<(CatalogReleaseIdentityV1, Arc<RacingVerifier>)>,
    retained_attempt_verifiers: Vec<(CatalogReleaseIdentityV1, Arc<RacingVerifier>)>,
    replay_slots: Arc<Semaphore>,
    ready: bool,
    service_verifier: Option<Arc<ServiceVerifier>>,
    retained_service_verifiers: Vec<Arc<ServiceVerifier>>,
}

struct ServiceVerifier {
    catalog_release: CatalogReleaseIdentityV1,
    verifier: RacingVerifier,
    replay: pitgun_verifier::wasm_service_replay::WasmServiceReplay,
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

async fn service_readyz(State(state): State<AppState>) -> StatusCode {
    if state.ready && state.service_verifier.is_some() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn admit_replay(slots: Arc<Semaphore>) -> Result<(i64, OwnedSemaphorePermit), AcquireError> {
    // JSON extraction has completed: bind timeliness to this server's receipt
    // of the complete evidence, not to later replay capacity. Never use a
    // caller-provided timestamp or extend the signed deadline.
    let received_at_ms = now_ms();
    let permit = slots.acquire_owned().await?;
    Ok((received_at_ms, permit))
}

// The supplied release selects only among operator-owned immutable snapshots.
// Every chosen engine still verifies the signed model/data-pack and full lineage.
// Unknown identities fall through to the primary engine's rejection, never to
// a fetch or a client-provided filesystem path. All engines share replay_slots.
fn select_verifier(
    primary: &Arc<RacingVerifier>,
    retained: &[(CatalogReleaseIdentityV1, Arc<RacingVerifier>)],
    release: Option<&CatalogReleaseIdentityV1>,
) -> Arc<RacingVerifier> {
    Arc::clone(
        retained
            .iter()
            .find(|(identity, _)| Some(identity) == release)
            .map(|(_, verifier)| verifier)
            .unwrap_or(primary),
    )
}

fn retained_verifiers(
    env_var: &str,
    primary: Option<&RacingCatalogSnapshot>,
    model: &ArtifactIdentity,
    template: &RacingVerifier,
) -> Result<Vec<(CatalogReleaseIdentityV1, Arc<RacingVerifier>)>, String> {
    let mut engines = Vec::new();
    if let Some(paths) = std::env::var_os(env_var) {
        for path in std::env::split_paths(&paths) {
            let catalog =
                RacingCatalogSnapshot::from_release_dir(path).map_err(|e| e.to_string())?;
            catalog
                .manifest()
                .compatibility
                .validate_for(model, pitgun_contract::ContractVersion::V1)
                .map_err(|e| e.to_string())?;
            let identity = catalog.release_identity().clone();
            if primary.is_some_and(|c| c.release_identity() == &identity)
                || engines.iter().any(|(id, _)| id == &identity)
            {
                return Err(format!("duplicate catalog identity in {env_var}"));
            }
            engines.push((identity, Arc::new(template.clone().with_catalog(catalog))));
        }
    }
    Ok(engines)
}

async fn verify_racing(
    State(state): State<AppState>,
    Json(submission): Json<RacingVerificationSubmissionV1>,
) -> Response {
    let (now_ms, permit) = match admit_replay(Arc::clone(&state.replay_slots)).await {
        Ok(admitted) => admitted,
        Err(error) => {
            error!(?error, "verifier replay semaphore is closed");
            return internal_error();
        }
    };
    let verifier = select_verifier(
        &state.verifier,
        &state.retained_verifiers,
        submission
            .execution_resolution
            .as_ref()
            .map(|r| &r.catalog_release),
    );
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

async fn verify_racing_attempt(
    State(state): State<AppState>,
    Json(submission): Json<RacingDynamicVerificationSubmissionV1>,
) -> Response {
    let (now_ms, permit) = match admit_replay(Arc::clone(&state.replay_slots)).await {
        Ok(admitted) => admitted,
        Err(error) => {
            error!(?error, "verifier replay semaphore is closed");
            return internal_error();
        }
    };
    let verifier = select_verifier(
        &state.attempt_verifier,
        &state.retained_attempt_verifiers,
        Some(&submission.execution_resolution.catalog_release),
    );
    let result = task::spawn_blocking(move || {
        let _permit = permit;
        verifier.verify_attempt(&submission, now_ms)
    })
    .await;

    match result {
        Ok(Ok(verdict)) => verdict_response(verdict),
        Ok(Err(error)) => {
            error!(?error, "dynamic Racing verification failed internally");
            internal_error()
        }
        Err(error) => {
            error!(?error, "dynamic Racing verification worker terminated");
            internal_error()
        }
    }
}

// Routing selects only operator-validated entries. It grants no trust: the
// selected verifier still validates the signature, receipt and entire history.
// Unknown pairs stay on the strict primary and are rejected before replay.
fn select_service_verifier(
    primary: Arc<ServiceVerifier>,
    retained: &[Arc<ServiceVerifier>],
    release: &CatalogReleaseIdentityV1,
    runtime: Digest,
) -> Arc<ServiceVerifier> {
    retained
        .iter()
        .find(|v| &v.catalog_release == release && v.replay.runtime_digest == runtime)
        .map(Arc::clone)
        .unwrap_or(primary)
}

fn retain_service_verifier(
    primary: &ServiceVerifier,
    retained: &mut Vec<Arc<ServiceVerifier>>,
    candidate: ServiceVerifier,
) -> Result<(), String> {
    if std::iter::once(primary)
        .chain(retained.iter().map(AsRef::as_ref))
        .any(|v| {
            v.catalog_release == candidate.catalog_release
                && v.replay.runtime_digest == candidate.replay.runtime_digest
        })
    {
        return Err("duplicate service catalog/runtime replay identity".into());
    }
    retained.push(Arc::new(candidate));
    Ok(())
}

async fn verify_racing_service(
    State(state): State<AppState>,
    Json(submission): Json<RacingServiceVerificationSubmissionV1>,
) -> Response {
    if !state.ready {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "verification unavailable",
            }),
        )
            .into_response();
    }
    let Some(verifier) = state.service_verifier else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "service control is not configured",
            }),
        )
            .into_response();
    };
    let verifier = select_service_verifier(
        verifier,
        &state.retained_service_verifiers,
        &submission.catalog_release,
        submission.decision_envelope.runtime_digest,
    );
    let (received, permit) = match admit_replay(Arc::clone(&state.replay_slots)).await {
        Ok(v) => v,
        Err(_) => return internal_error(),
    };
    match task::spawn_blocking(move || {
        let _permit = permit;
        verifier.verifier.verify_service_with_replay(
            &submission,
            received,
            |response, catalog, history| verifier.replay.replay(response, catalog, history),
        )
    })
    .await
    {
        Ok(Ok(verdict)) => verdict_response(verdict),
        Ok(Err(error)) => {
            error!(?error, "service verification failed internally");
            internal_error()
        }
        Err(error) => {
            error!(?error, "service verification worker terminated");
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
        .route(
            "/v1/verifications/racing/attempts",
            post(verify_racing_attempt),
        )
        .route(
            "/v1/verifications/racing/services",
            post(verify_racing_service).layer(DefaultBodyLimit::max(2 * 1024 * 1024)),
        )
        .route(
            "/v1/verifications/racing/services/readyz",
            get(service_readyz),
        )
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

fn load_catalog(env_var: &str, model_version: &str) -> Option<RacingCatalogSnapshot> {
    let loaded = match std::env::var_os(env_var) {
        None if model_version == "0.16.1" => RacingCatalogSnapshot::embedded_qualified_career(),
        Some(path) => RacingCatalogSnapshot::from_release_dir(PathBuf::from(path)),
        None if model_version == "0.15.0" => {
            RacingCatalogSnapshot::embedded_model_v3_fuel_contract()
        }
        None if model_version == "0.14.0" => RacingCatalogSnapshot::embedded_model_v3_timeline(),
        None if model_version == "0.10.0" => RacingCatalogSnapshot::embedded_model_v3_thermal(),
        None if model_version == "2.0.0" => RacingCatalogSnapshot::embedded_model_v2(),
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
    let model_version = std::env::var("PITGUN_RACING_MODEL_VERSION")
        .unwrap_or_else(|_| DEFAULT_RACING_MODEL_VERSION.to_owned());
    let expected_model = racing_model_identity_for_version(&model_version)?;
    let attempt_model_version = std::env::var("PITGUN_RACING_ATTEMPT_MODEL_VERSION")
        .unwrap_or_else(|_| DEFAULT_RACING_ATTEMPT_MODEL_VERSION.to_owned());
    let expected_attempt_model = racing_model_identity_for_version(&attempt_model_version)?;
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
    let catalog =
        load_catalog("PITGUN_RACING_CATALOG_RELEASE_DIR", &model_version).and_then(|catalog| {
            match catalog
                .manifest()
                .compatibility
                .validate_for(&expected_model, pitgun_contract::ContractVersion::V1)
            {
                Ok(()) => Some(catalog),
                Err(error) => {
                    error!(?error, "configured Racing model/catalog pair is invalid");
                    None
                }
            }
        });
    let attempt_catalog = load_catalog(
        "PITGUN_RACING_ATTEMPT_CATALOG_RELEASE_DIR",
        &attempt_model_version,
    )
    .and_then(|catalog| {
        match catalog.manifest().compatibility.validate_for(
            &expected_attempt_model,
            pitgun_contract::ContractVersion::V1,
        ) {
            Ok(()) => Some(catalog),
            Err(error) => {
                error!(
                    ?error,
                    "configured dynamic Racing model/catalog pair is invalid"
                );
                None
            }
        }
    });
    let ready = signing_key.is_some() && catalog.is_some() && attempt_catalog.is_some();
    let verifier_identity = load_verifier_identity()?;
    let service_catalog_path = std::env::var_os("PITGUN_RACING_SERVICE_CATALOG_RELEASE_DIR");
    let service_runtime = std::env::var("PITGUN_RACING_SERVICE_WASM_DIGEST").ok();
    let service_verifier = match (service_catalog_path, service_runtime) {
        (None, None) => None,
        (Some(path), Some(digest)) => {
            let service_catalog = RacingCatalogSnapshot::from_release_dir(PathBuf::from(path))
                .map_err(|e| e.to_string())?;
            let service_model = if service_catalog.qualified_career_profile_bytes().is_some() {
                pitgun_racing_simulator::qualified_career::model_identity()
            } else {
                pitgun_racing_simulator::racing_model_v3_component_candidate_identity()
            };
            service_catalog
                .manifest()
                .compatibility
                .validate_for(&service_model, pitgun_contract::ContractVersion::V1)
                .map_err(|e| e.to_string())?;
            use pitgun_verifier::wasm_service_replay::WasmServiceReplay;
            let required = |key: &str| {
                std::env::var(key).map_err(|_| format!("missing {key} for service WASM replay"))
            };
            let runtime_digest = digest.parse::<Digest>().map_err(|e| e.to_string())?;
            let replay = WasmServiceReplay {
                node: PathBuf::from(required("PITGUN_SERVICE_NODE_BINARY")?),
                script: PathBuf::from(required("PITGUN_SERVICE_REPLAY_SCRIPT")?),
                wasm: PathBuf::from(required("PITGUN_SERVICE_WASM_FILE")?),
                bridge: PathBuf::from(required("PITGUN_SERVICE_BRIDGE_FILE")?),
                runtime_digest,
                bridge_digest: required("PITGUN_SERVICE_BRIDGE_DIGEST")?
                    .parse::<Digest>()
                    .map_err(|e| e.to_string())?,
                timeout: pitgun_verifier::wasm_service_replay::replay_timeout(
                    std::env::var("PITGUN_SERVICE_REPLAY_TIMEOUT_SECONDS")
                        .ok()
                        .as_deref(),
                )?,
            };
            replay.validate()?;
            let identity = replay.verifier_identity(&verifier_identity)?;
            Some(Arc::new(ServiceVerifier {
                catalog_release: service_catalog.release_identity().clone(),
                verifier: RacingVerifier::new(
                    keyring.clone(),
                    expected_audience.clone(),
                    service_model,
                    policy.clone(),
                    Some(service_catalog),
                    identity,
                )
                .with_service_runtime(runtime_digest),
                replay,
            }))
        }
        _ => {
            return Err(
                "service control requires both compatible catalog and runtime digest".into(),
            );
        }
    };
    let mut retained_service_verifiers = Vec::new();
    if let Some(path) = std::env::var_os("PITGUN_RETAINED_SERVICE_REPLAYS_FILE") {
        let primary = service_verifier
            .as_ref()
            .ok_or("retained service replays require a configured primary service verifier")?;
        let manifest = service_replay_config::RetainedReplays::load(PathBuf::from(path))?;
        for config in manifest.replays {
            let catalog = RacingCatalogSnapshot::from_release_dir(config.catalog_release_dir)
                .map_err(|e| e.to_string())?;
            let service_model = if catalog.qualified_career_profile_bytes().is_some() {
                pitgun_racing_simulator::qualified_career::model_identity()
            } else {
                pitgun_racing_simulator::racing_model_v3_component_candidate_identity()
            };
            catalog
                .manifest()
                .compatibility
                .validate_for(&service_model, pitgun_contract::ContractVersion::V1)
                .map_err(|e| e.to_string())?;
            let replay = pitgun_verifier::wasm_service_replay::WasmServiceReplay {
                node: config.node,
                script: config.script,
                wasm: config.wasm,
                bridge: config.bridge,
                runtime_digest: config.runtime_digest,
                bridge_digest: config.bridge_digest,
                timeout: pitgun_verifier::wasm_service_replay::replay_timeout(Some(
                    &config.timeout_seconds.to_string(),
                ))?,
            };
            replay.validate()?;
            let identity = replay.verifier_identity(&verifier_identity)?;
            let candidate = ServiceVerifier {
                catalog_release: catalog.release_identity().clone(),
                verifier: RacingVerifier::new(
                    keyring.clone(),
                    expected_audience.clone(),
                    service_model,
                    policy.clone(),
                    Some(catalog),
                    identity,
                )
                .with_service_runtime(config.runtime_digest),
                replay,
            };
            retain_service_verifier(primary, &mut retained_service_verifiers, candidate)?;
        }
    }
    let verifier = RacingVerifier::new(
        keyring.clone(),
        expected_audience.clone(),
        expected_model.clone(),
        policy.clone(),
        catalog.clone(),
        verifier_identity.clone(),
    );
    let attempt_verifier = RacingVerifier::new(
        keyring,
        expected_audience,
        expected_attempt_model.clone(),
        policy,
        attempt_catalog.clone(),
        verifier_identity,
    );

    let retained_verifiers = retained_verifiers(
        "PITGUN_RACING_RETAINED_CATALOG_RELEASE_DIRS",
        catalog.as_ref(),
        &expected_model,
        &verifier,
    )?;
    let retained_attempt_verifiers = self::retained_verifiers(
        "PITGUN_RACING_ATTEMPT_RETAINED_CATALOG_RELEASE_DIRS",
        attempt_catalog.as_ref(),
        &expected_attempt_model,
        &attempt_verifier,
    )?;
    Ok(AppState {
        retained_verifiers,
        retained_attempt_verifiers,
        verifier: Arc::new(verifier),
        attempt_verifier: Arc::new(attempt_verifier),
        replay_slots: Arc::new(Semaphore::new(parse_max_concurrent_replays())),
        ready,
        service_verifier,
        retained_service_verifiers,
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
        ArtifactIdentity, AuthorizationValidityV1, Digest, ExecutionId, SubmittedEvidenceV1,
        VerificationReasonCode, VerificationStatus, VerificationVerdictV1,
        VerificationVerdictVersion,
    };
    use std::{future::Future, task::Poll, time::Duration};

    use super::verdict_response;

    #[test]
    fn routing_uses_exact_operator_release_and_unknown_stays_on_strict_primary() {
        use super::*;
        let catalog = RacingCatalogSnapshot::from_release_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.8.1"),
        )
        .unwrap();
        let identity = catalog.release_identity().clone();
        let primary = Arc::new(RacingVerifier::new(
            VerificationKeyring::new(),
            "pitgun.verifier".parse().unwrap(),
            racing_model_identity_for_version("0.11.0").unwrap(),
            verdict(VerificationStatus::Verified).verifier,
            None,
            verdict(VerificationStatus::Verified).verifier,
        ));
        let other = Arc::new(primary.as_ref().clone().with_catalog(catalog));
        let retained = vec![(identity.clone(), other.clone())];
        assert!(Arc::ptr_eq(
            &select_verifier(&primary, &retained, Some(&identity)),
            &other
        ));
        let mut wrong = identity;
        wrong.manifest_digest = Digest::from_bytes(b"unknown");
        assert!(Arc::ptr_eq(
            &select_verifier(&primary, &retained, Some(&wrong)),
            &primary
        ));
        assert!(Arc::ptr_eq(
            &select_verifier(&primary, &retained, None),
            &primary
        ));
    }

    #[test]
    fn service_routing_matches_catalog_and_runtime_and_refuses_duplicate_pairs() {
        use super::*;
        let catalog = RacingCatalogSnapshot::from_release_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.8.1"),
        )
        .unwrap();
        let release = catalog.release_identity().clone();
        let make = |runtime: Digest| ServiceVerifier {
            catalog_release: release.clone(),
            verifier: RacingVerifier::new(
                VerificationKeyring::new(),
                "pitgun.verifier".parse().unwrap(),
                pitgun_racing_simulator::racing_model_v3_component_candidate_identity(),
                verdict(VerificationStatus::Verified).verifier,
                Some(catalog.clone()),
                verdict(VerificationStatus::Verified).verifier,
            )
            .with_service_runtime(runtime),
            replay: pitgun_verifier::wasm_service_replay::WasmServiceReplay {
                node: "/not-used/node".into(),
                script: "/not-used/script".into(),
                wasm: "/not-used/engine.wasm".into(),
                bridge: "/not-used/bridge.mjs".into(),
                runtime_digest: runtime,
                bridge_digest: Digest::from_bytes(b"bridge"),
                timeout: Duration::from_secs(120),
            },
        };
        let current = Digest::from_bytes(b"current");
        let old = Digest::from_bytes(b"retained");
        let primary = Arc::new(make(current));
        let mut retained = Vec::new();
        retain_service_verifier(&primary, &mut retained, make(old)).unwrap();
        assert!(Arc::ptr_eq(
            &select_service_verifier(primary.clone(), &retained, &release, old),
            &retained[0]
        ));
        assert!(Arc::ptr_eq(
            &select_service_verifier(primary.clone(), &retained, &release, current),
            &primary
        ));
        assert!(Arc::ptr_eq(
            &select_service_verifier(
                primary.clone(),
                &retained,
                &release,
                Digest::from_bytes(b"unknown")
            ),
            &primary
        ));
        let mut wrong = release.clone();
        wrong.manifest_digest = Digest::from_bytes(b"unknown catalog");
        assert!(Arc::ptr_eq(
            &select_service_verifier(primary.clone(), &retained, &wrong, old),
            &primary
        ));
        assert!(retain_service_verifier(&primary, &mut retained, make(old)).is_err());
        assert!(retain_service_verifier(&primary, &mut retained, make(current)).is_err());
        assert_eq!(retained.len(), 1);
    }

    #[tokio::test]
    async fn capacity_wait_does_not_expire_evidence_received_in_time() {
        let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let busy = slots.clone().acquire_owned().await.unwrap();
        let mut admission = Box::pin(super::admit_replay(slots));
        let issued_at_ms = super::now_ms();
        // Poll once to enter the actual capacity queue, without granting a slot.
        let queued = std::future::poll_fn(|cx| Poll::Ready(admission.as_mut().poll(cx))).await;
        assert!(queued.is_pending());
        let deadline = super::now_ms() + 1;
        let validity = AuthorizationValidityV1 {
            issued_at_ms,
            expires_at_ms: deadline,
            late_submission_grace_ms: 0,
        };
        std::thread::sleep(Duration::from_millis(20));
        assert!(validity.validate_submission_time(super::now_ms()).is_err());
        drop(busy);
        let (received_at, _permit) = admission.await.unwrap();
        assert!(validity.validate_submission_time(received_at).is_ok());
    }

    #[tokio::test]
    async fn expired_at_receipt_remains_expired_without_waiting() {
        let validity = AuthorizationValidityV1 {
            issued_at_ms: super::now_ms() - 2000,
            expires_at_ms: super::now_ms() - 1000,
            late_submission_grace_ms: 0,
        };
        let (received_at, _permit) =
            super::admit_replay(std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
                .await
                .unwrap();
        assert!(validity.validate_submission_time(received_at).is_err());
    }

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
